use std::collections::HashSet;

use eyre::Result;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use tracing::info;

use crate::github::get_log_futures;
use crate::github::CheckConclusionState;
use crate::github::GithubClient;
use crate::repo_config::TypecheckConfig;
use crate::term::print_check_run_header;
use crate::{git::Repository, github, repo_config::RepoConfig, term::green};

use self::tsc::TscLogParser;

mod tsc;

pub async fn typecheck(
    client: &GithubClient,
    repo: &Repository,
    branch: &str,
    repo_config: &RepoConfig,
    show_files_only: bool,
) -> Result<()> {
    let typecheck_config = repo_config
        .typecheck
        .as_ref()
        .ok_or_else(|| eyre::eyre!("No typecheck config found in .ghtool.toml"))?;

    let pr = client
        .get_pr_for_branch_memoized(&repo.owner, &repo.name, branch)
        .await?;

    let check_runs = client.get_pr_status_checks(&pr.id).await?;
    let (typecheck_check_runs, any_in_progress) =
        filter_typecheck_runs(check_runs, typecheck_config);
    info!(?typecheck_check_runs, "got typecheck check runs");

    if typecheck_check_runs.is_empty() {
        eprintln!(
            "No typechecking jobs found matching the pattern /{}/",
            typecheck_config.job_pattern
        );
    } else {
        process_failing_checks(
            client,
            repo,
            typecheck_check_runs,
            any_in_progress,
            show_files_only,
        )
        .await?;
    }

    Ok(())
}

async fn process_failing_checks(
    client: &GithubClient,
    repo: &Repository,
    typecheck_check_runs: Vec<github::SimpleCheckRun>,
    any_in_progress: bool,
    show_files_only: bool,
) -> Result<()> {
    let failing_typecheck_check_runs: Vec<_> = typecheck_check_runs
        .into_iter()
        .filter(|cr| cr.conclusion == Some(CheckConclusionState::Failure))
        .collect();

    if failing_typecheck_check_runs.is_empty() {
        if any_in_progress {
            eprintln!("⏳  Some typecheck jobs are in progress");
        } else {
            eprintln!("{}  All typecheck jobs are green", green("✓"));
        }
        return Ok(());
    }

    if show_files_only {
        get_failing_typechecks_files(client, repo, failing_typecheck_check_runs).await?;
    } else {
        get_failing_typechecks(client, repo, failing_typecheck_check_runs).await?;
    }

    Ok(())
}

fn filter_typecheck_runs(
    check_runs: Vec<github::SimpleCheckRun>,
    typecheck_config: &TypecheckConfig,
) -> (Vec<github::SimpleCheckRun>, bool) {
    let mut typecheck_check_runs = Vec::new();
    let mut any_in_progress = false;

    for cr in check_runs {
        if typecheck_config.job_pattern.is_match(&cr.name) {
            if cr.conclusion.is_none() {
                any_in_progress = true;
            }
            typecheck_check_runs.push(cr);
        }
    }

    (typecheck_check_runs, any_in_progress)
}

async fn get_failing_typechecks(
    client: &GithubClient,
    repo: &Repository,
    failing_typecheck_check_runs: Vec<github::SimpleCheckRun>,
) -> Result<()> {
    let mut log_futures: FuturesUnordered<_> =
        get_log_futures(client, repo, &failing_typecheck_check_runs);

    let mut failing_typechecks = Vec::new();
    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let logs = String::from_utf8_lossy(&bytes);
        let failing_typechecks_inner = TscLogParser::parse(&logs)?;
        failing_typechecks.push(failing_typechecks_inner);
    }

    if failing_typechecks.iter().all(|s| s.is_empty()) {
        eprintln!("No type errors found in log output");
        return Ok(());
    }

    for (check_run, failing_typecheck) in
        failing_typecheck_check_runs.iter().zip(failing_typechecks)
    {
        print_check_run_header(check_run);

        for tsc_error in failing_typecheck {
            for line in tsc_error.lines {
                println!("{}", line);
            }
        }
    }

    Ok(())
}

async fn get_failing_typechecks_files(
    client: &GithubClient,
    repo: &Repository,
    failing_typecheck_check_runs: Vec<github::SimpleCheckRun>,
) -> Result<()> {
    let mut log_futures: FuturesUnordered<_> =
        get_log_futures(client, repo, &failing_typecheck_check_runs);

    let mut failing_typecheck_files: HashSet<String> = HashSet::new();

    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let logs = String::from_utf8_lossy(&bytes);
        let parsed = TscLogParser::parse(&logs)?;
        for tsc_error in parsed {
            failing_typecheck_files.insert(tsc_error.path);
        }
    }

    if failing_typecheck_files.is_empty() {
        eprintln!("No type errors found in log output");
        return Ok(());
    }

    for file in failing_typecheck_files {
        println!("{}", file);
    }

    Ok(())
}
