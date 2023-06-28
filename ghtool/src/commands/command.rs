use std::collections::HashSet;

use async_trait::async_trait;
use eyre::Result;
use futures::{stream::FuturesUnordered, StreamExt};
use regex::Regex;
use tracing::info;

use crate::{
    git::Repository,
    github::{get_log_futures, CheckConclusionState, GithubClient, SimpleCheckRun},
    term::{green, print_check_run_header},
};

pub trait ConfigPattern {
    fn job_pattern(&self) -> &Regex;
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckError {
    pub path: String,
    pub lines: Vec<String>,
}

#[async_trait]
pub trait Command {
    type ConfigType: ConfigPattern;

    fn name(&self) -> &'static str;
    fn config(&self) -> &Self::ConfigType;
    fn parse_log(&self, logs: &str) -> Result<Vec<CheckError>>;
    fn check_error_plural(&self) -> &'static str;
}

fn filter_check_runs<T: Command>(
    command: &T,
    check_runs: Vec<SimpleCheckRun>,
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
                failed_check_runs.push(run);
            }
        }
    }

    (failed_check_runs, any_in_progress, no_matching_runs)
}

pub async fn handle_command<T: Command>(
    command: T,
    client: &GithubClient,
    repo: &Repository,
    branch: &str,
    show_files_only: bool,
) -> Result<()> {
    let pull_request = client
        .get_pr_for_branch_memoized(&repo.owner, &repo.name, branch)
        .await?;

    let all_check_runs = client.get_pr_status_checks(&pull_request.id).await?;
    let (failed_check_runs, any_in_progress, no_matching_runs) =
        filter_check_runs(&command, all_check_runs);
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
        if any_in_progress {
            eprintln!("⏳  Some {} checks are in progress", command.name());
        } else {
            eprintln!("{}  All checks are green", green("✓"));
        }
        return Ok(());
    }

    let mut log_futures: FuturesUnordered<_> = get_log_futures(client, repo, &failed_check_runs);
    let mut all_checks_errors = Vec::new();
    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let log = String::from_utf8_lossy(&bytes);
        let check_errors = command.parse_log(&log)?;
        all_checks_errors.push(check_errors);
    }

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
