use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use eyre::Result;
use futures::future::try_join_all;
use regex::Regex;
use tokio::task::JoinHandle;
use tracing::info;

use crate::{
    cli::Cli,
    commands::{BuildCommand, LintCommand, TestCommand},
    git::Repository,
    github::{
        fetch_check_run_logs, wait_for_pr_checks, CheckConclusionState, GithubClient,
        SimpleCheckRun,
    },
    repo_config::RepoConfig,
    setup::get_repo_config,
    term::{bold, print_all_checks_green, print_check_run_header},
    token_store,
};

pub trait ConfigPattern {
    fn job_pattern(&self) -> &Regex;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckError {
    pub path: String,
    pub lines: Vec<String>,
}

pub trait Command: Sync + Send {
    fn name(&self) -> &'static str;
    fn config(&self) -> &dyn ConfigPattern;
    fn check_error_plural(&self) -> &'static str;
    fn parse_log(&self, logs: &str) -> Result<Vec<CheckError>>;
}

fn filter_check_runs(
    command: &dyn Command,
    check_runs: &[SimpleCheckRun],
) -> (Vec<SimpleCheckRun>, bool, bool) {
    let mut failed_check_runs = Vec::new();
    let mut any_in_progress = false;
    let mut no_matching_runs = true;

    for run in check_runs {
        if command.config().job_pattern().is_match(&run.name) {
            no_matching_runs = false;

            if run.conclusion.is_none() {
                any_in_progress = true;
            }

            if run.conclusion == Some(CheckConclusionState::Failure) {
                failed_check_runs.push(run.clone());
            }
        }
    }

    (failed_check_runs, any_in_progress, no_matching_runs)
}

pub async fn handle_command(
    command_type: CommandType,
    cli: &Cli,
    show_files_only: bool,
) -> Result<()> {
    let (repo_config, repo, branch) = get_repo_config(cli)?;
    let command = command_from_type(command_type, &repo_config)?;
    let token = get_token(&repo.hostname)?;
    let client = GithubClient::new(&token)?;
    let pull_request = client
        .get_pr_for_branch_memoized(&repo.owner, &repo.name, &branch)
        .await?
        .ok_or_else(|| eyre::eyre!("No pull request found for branch {}", bold(&branch)))?;

    let command_clone = command.clone();
    let match_checkrun_name =
        move |name: &str| -> bool { command_clone.config().job_pattern().is_match(name) };

    let all_check_runs =
        wait_for_pr_checks(&client, pull_request.id, Some(&match_checkrun_name)).await?;

    let (failed_check_runs, _, no_matching_runs) = filter_check_runs(&*command, &all_check_runs);
    info!(?failed_check_runs, "got failed check runs");

    if no_matching_runs {
        eprintln!(
            "No {} jobs found matching the pattern /{}/",
            command.name(),
            command.config().job_pattern()
        );
        return Ok(());
    }

    if failed_check_runs.is_empty() {
        print_all_checks_green();
        return Ok(());
    }

    let check_run_errors = process_failed_check_runs(
        &client,
        &repo,
        CommandMode::Single(command.clone()),
        &failed_check_runs,
    )
    .await?;

    let all_checks_errors = check_run_errors.into_values().collect::<Vec<_>>();
    if all_checks_errors.iter().all(|s| s.is_empty()) {
        eprintln!("No {} found in log output", command.check_error_plural());
        return Ok(());
    }

    if show_files_only {
        print_errored_files(all_checks_errors);
    } else {
        print_errors(&failed_check_runs, all_checks_errors);
    }

    Ok(())
}

#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
pub enum CommandType {
    Test,
    Lint,
    Build,
}

pub async fn handle_all_command(cli: &Cli) -> Result<()> {
    let (repo_config, repo, branch) = get_repo_config(cli)?;
    let token = get_token(&repo.hostname)?;
    let client = GithubClient::new(&token)?;
    let pull_request = client
        .get_pr_for_branch_memoized(&repo.owner, &repo.name, &branch)
        .await?
        .ok_or_else(|| eyre::eyre!("No pull request found for branch {}", bold(&branch)))?;

    let all_check_runs = wait_for_pr_checks(&client, pull_request.id, None).await?;
    let mut all_failed_check_runs = Vec::new();
    let mut check_run_command_map: HashMap<CheckRunId, CommandType> = HashMap::new();
    let mut command_check_run_map: HashMap<CommandType, Vec<CheckRunId>> = HashMap::new();

    let command_types = [CommandType::Test, CommandType::Build, CommandType::Lint];
    let commands: Result<HashMap<CommandType, Arc<dyn Command + Send + Sync>>> = command_types
        .iter()
        .map(|&command_type| Ok((command_type, command_from_type(command_type, &repo_config)?)))
        .collect();
    let commands = commands?;

    for (command_type, command) in &commands {
        add_command_info(
            command.as_ref(),
            *command_type,
            &all_check_runs,
            &mut all_failed_check_runs,
            &mut check_run_command_map,
            &mut command_check_run_map,
        );
    }

    let mut all_check_errors = process_failed_check_runs(
        &client,
        &repo,
        CommandMode::Multiple {
            command_map: commands,
            check_run_command_map,
        },
        &all_failed_check_runs,
    )
    .await?;

    let mut all_green = true;
    for command_type in &[CommandType::Test, CommandType::Build, CommandType::Lint] {
        let check_run_ids = command_check_run_map
            .remove(command_type)
            .unwrap_or_default();
        let check_runs: Vec<_> = check_run_ids
            .iter()
            .filter_map(|&id| all_check_runs.iter().find(|&run| run.id == id).cloned())
            .collect();

        let mut check_errors = Vec::new();
        for check_run_id in &check_run_ids {
            if let Some(errors) = all_check_errors.remove(check_run_id) {
                check_errors.push(errors);
            }
        }

        if check_errors.iter().all(|s| s.is_empty()) {
            continue;
        }

        all_green = false;
        print_errors(&check_runs, check_errors);
    }

    if all_green {
        print_all_checks_green();
    }

    Ok(())
}

fn command_from_type(
    command_type: CommandType,
    repo_config: &RepoConfig,
) -> Result<Arc<dyn Command + Send + Sync>> {
    let command: Box<dyn Command + Send + Sync> = match command_type {
        CommandType::Test => Box::new(TestCommand::from_repo_config(repo_config)?),
        CommandType::Build => Box::new(BuildCommand::from_repo_config(repo_config)?),
        CommandType::Lint => Box::new(LintCommand::from_repo_config(repo_config)?),
    };
    Ok(Arc::from(command))
}

fn print_errored_files(all_checks_errors: Vec<Vec<CheckError>>) {
    let files: HashSet<String> = all_checks_errors
        .into_iter()
        .flat_map(|errors| errors.into_iter().map(|error| error.path))
        .collect();

    for file in files {
        println!("{}", file);
    }
}

fn print_errors(failed_check_runs: &[SimpleCheckRun], all_checks_errors: Vec<Vec<CheckError>>) {
    failed_check_runs
        .iter()
        .zip(all_checks_errors)
        .for_each(|(check_run, check_errors)| {
            print_check_run_header(check_run);

            check_errors
                .into_iter()
                .flat_map(|error| error.lines)
                .for_each(|line| println!("{}", line));
        });
}

type CheckRunId = u64;

enum CommandMode {
    Single(Arc<dyn Command + Send + Sync>),
    Multiple {
        // Used to provide command's parse log function
        command_map: HashMap<CommandType, Arc<dyn Command + Send + Sync>>,
        // Used to determine how check run's logs should be parsed
        check_run_command_map: HashMap<CheckRunId, CommandType>,
    },
}

/// Get logs for each failed check run, and parse them into a map of command type to check errors
async fn process_failed_check_runs(
    client: &GithubClient,
    repo: &Repository,
    command_mode: CommandMode,
    all_failed_check_runs: &[SimpleCheckRun],
) -> Result<HashMap<CheckRunId, Vec<CheckError>>> {
    let log_map = fetch_check_run_logs(client, repo, all_failed_check_runs).await?;
    #[allow(clippy::type_complexity)]
    let mut parse_futures: Vec<JoinHandle<Result<(CheckRunId, Vec<CheckError>)>>> = Vec::new();

    for (check_run_id, log_bytes) in log_map.iter() {
        let check_run_id = *check_run_id;
        let log_bytes = log_bytes.clone();
        let command = match &command_mode {
            CommandMode::Single(single_command) => {
                single_command.clone() // Single mode: use the same command for all check runs
            }
            CommandMode::Multiple {
                command_map,
                check_run_command_map,
            } => {
                let command_type = check_run_command_map
                    .get(&check_run_id)
                    .unwrap_or_else(|| panic!("Unknown check run id: {}", check_run_id));
                command_map.get(command_type).unwrap().clone()
            }
        };

        let handle = tokio::task::spawn_blocking(move || {
            let log_str = std::str::from_utf8(&log_bytes)?;
            Ok((check_run_id, command.parse_log(log_str)?))
        });
        parse_futures.push(handle);
    }

    let results = try_join_all(parse_futures).await?;
    let mut check_errors_map = HashMap::new();
    for result in results {
        let (command_type, check_errors) = result?;
        check_errors_map
            .entry(command_type)
            .or_insert_with(Vec::new)
            .extend(check_errors);
    }

    Ok(check_errors_map)
}

fn get_token(hostname: &str) -> Result<String> {
    // In development, macOS is constantly asking for password when token store is accessed with a
    // new binary
    if let Ok(token) = std::env::var("GH_TOKEN") {
        return Ok(token);
    }

    token_store::get_token(hostname).map_err(|err| match err {
        keyring::Error::NoEntry => {
            eyre::eyre!(
                "No token found for {}. Have you logged in? Run {}",
                bold(hostname),
                bold("ghtool login")
            )
        }
        err => eyre::eyre!("Failed to get token for {}: {}", hostname, err),
    })
}

fn add_command_info(
    command: &dyn Command,
    command_type: CommandType,
    all_check_runs: &[SimpleCheckRun],
    all_failed_check_runs: &mut Vec<SimpleCheckRun>,
    check_run_command_map: &mut HashMap<u64, CommandType>,
    command_check_run_map: &mut HashMap<CommandType, Vec<u64>>,
) {
    let (failed, _, _) = filter_check_runs(command, all_check_runs);
    all_failed_check_runs.extend_from_slice(&failed);

    for check_run in &failed {
        check_run_command_map.insert(check_run.id, command_type);
        command_check_run_map
            .entry(command_type)
            .or_default()
            .push(check_run.id);
    }
}
