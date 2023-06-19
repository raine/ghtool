mod eslint;

use eslint::*;
use eyre::Result;
use futures::{stream::FuturesUnordered, StreamExt};
use tracing::info;

use crate::{
    git::Repository,
    github::{self, get_log_futures, CheckConclusionState, GithubClient},
    repo_config::RepoConfig,
    term::{green, print_check_run_header},
};

pub async fn lint(
    client: &GithubClient,
    repo: &Repository,
    branch: &str,
    repo_config: &RepoConfig,
    show_files_only: bool,
) -> Result<()> {
    let pr = client
        .get_pr_for_branch_memoized(&repo.owner, &repo.name, branch)
        .await?;
    let check_runs = client.get_pr_status_checks(&pr.id).await?;
    let (lint_check_runs, any_in_progress) = filter_lint_check_runs(check_runs, repo_config);
    info!(?lint_check_runs, "got lint check runs");

    if lint_check_runs.is_empty() {
        eprintln!(
            "No lint check runs found matching the pattern /{}/",
            repo_config.lint_job_pattern
        );
    } else {
        process_failing_checks(
            client,
            repo,
            lint_check_runs,
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
    lint_check_runs: Vec<github::SimpleCheckRun>,
    any_in_progress: bool,
    show_files_only: bool,
) -> Result<()> {
    let failing_lint_check_runs: Vec<_> = lint_check_runs
        .into_iter()
        .filter(|cr| cr.conclusion == Some(CheckConclusionState::Failure))
        .collect();

    if failing_lint_check_runs.is_empty() {
        if any_in_progress {
            eprintln!("⏳  Some lint checks are in progress");
        } else {
            eprintln!("{}  All lint checks are green", green("✓"));
        }
        return Ok(());
    }

    let mut log_futures: FuturesUnordered<_> =
        get_log_futures(client, repo, &failing_lint_check_runs);

    let mut all_failing_lint_checks = Vec::new();
    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let log = String::from_utf8_lossy(&bytes);
        let output = EslintLogParser::parse(&log);
        all_failing_lint_checks.push(output);
    }

    if all_failing_lint_checks.iter().all(|s| s.is_empty()) {
        eprintln!("No failing lint checks found in log output");
        return Ok(());
    }

    if show_files_only {
        print_failed_lint_files(&all_failing_lint_checks);
    } else {
        print_failed_lint_issues(&failing_lint_check_runs, &all_failing_lint_checks);
    }

    Ok(())
}

fn print_failed_lint_files(all_failing_lint_checks: &[Vec<EslintPath>]) {
    for failing_lint_check in all_failing_lint_checks {
        for eslint_path in failing_lint_check {
            println!("{}", eslint_path.path);
        }
    }
}

fn print_failed_lint_issues(
    failing_lint_check_runs: &[github::SimpleCheckRun],
    all_failing_lint_checks: &[Vec<EslintPath>],
) {
    for (check_run, failing_lint_check) in
        failing_lint_check_runs.iter().zip(all_failing_lint_checks)
    {
        print_check_run_header(check_run);

        let mut iter = failing_lint_check.iter().peekable();
        while let Some(eslint_path) = iter.next() {
            println!("{}", eslint_path.path);

            for issue in &eslint_path.issues {
                println!("{}", issue);
            }

            if iter.peek().is_some() {
                println!();
            }
        }
    }
}

fn filter_lint_check_runs(
    check_runs: Vec<github::SimpleCheckRun>,
    repo_config: &RepoConfig,
) -> (Vec<github::SimpleCheckRun>, bool) {
    let mut lint_check_runs = Vec::new();
    let mut any_in_progress = false;

    for cr in check_runs {
        if repo_config.lint_job_pattern.is_match(&cr.name) {
            if cr.conclusion.is_none() {
                any_in_progress = true;
            }
            lint_check_runs.push(cr);
        }
    }

    (lint_check_runs, any_in_progress)
}
