use std::collections::HashSet;

use eyre::Result;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use github::graphql::CheckConclusionState;

use crate::{
    git::Repository,
    github,
    repo_config::RepoConfig,
    term::{bold, exit_with_message, green, print_header},
};

mod log_parsing;

use log_parsing::*;

pub async fn failing_tests(
    repo: &Repository,
    branch: &str,
    repo_config: &RepoConfig,
    show_files_only: bool,
) -> Result<()> {
    let pr = github::get_pr_for_branch_memoized(repo, branch).await?;
    let pr_id = pr.node_id.expect("pr node_id is missing");
    let check_runs = github::get_pr_status_checks(&pr_id).await?;
    let (test_check_runs, any_in_progress) = filter_test_runs(&check_runs, repo_config);

    if test_check_runs.is_empty() {
        eprintln!(
            "No test jobs found matching the pattern /{}/",
            repo_config.test_job_pattern
        );
    } else if any_in_progress {
        eprintln!("No failed test runs, but some checks are still pending");
    } else {
        process_failing_runs(repo, test_check_runs, show_files_only).await?;
    }

    Ok(())
}

async fn process_failing_runs(
    repo: &Repository,
    test_check_runs: Vec<&github::CheckRun>,
    show_files_only: bool,
) -> Result<()> {
    let failing_test_check_runs: Vec<_> = test_check_runs
        .into_iter()
        .filter(|cr| cr.conclusion == Some(CheckConclusionState::Failure))
        .collect();

    if failing_test_check_runs.is_empty() {
        exit_with_message(0, &format!("{}  All test checks are green", green("✓")));
    } else if show_files_only {
        get_failing_test_files(repo, failing_test_check_runs).await?;
    } else {
        get_failing_tests(repo, failing_test_check_runs).await?;
    }

    Ok(())
}

fn filter_test_runs<'a>(
    check_runs: &'a [github::CheckRun],
    repo_config: &'a RepoConfig,
) -> (Vec<&'a github::CheckRun>, bool) {
    let mut test_check_runs = Vec::new();
    let mut any_in_progress = false;

    for cr in check_runs {
        if repo_config.test_job_pattern.is_match(&cr.name) {
            test_check_runs.push(cr);
            if cr.conclusion.is_none() {
                any_in_progress = true;
            }
        }
    }

    (test_check_runs, any_in_progress)
}

pub async fn get_failing_tests(
    repo: &Repository,
    failing_test_check_runs: Vec<&github::CheckRun>,
) -> Result<()> {
    let mut log_futures: FuturesUnordered<_> = failing_test_check_runs
        .iter()
        .map(|cr| github::get_job_logs(repo, cr))
        .collect();

    let mut failing_tests = Vec::new();

    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let logs = String::from_utf8_lossy(&bytes);
        let failing_tests_inner = extract_failing_tests(&logs)?;
        failing_tests.push(failing_tests_inner);
    }

    if failing_tests.iter().all(|s| s.is_empty()) {
        eprintln!("No failing tests found in log output");
        return Ok(());
    }

    for (check_run, failing_test) in failing_test_check_runs.iter().zip(failing_tests) {
        print_header(&format!(
            "{} {}\n{} {}",
            bold("Job:"),
            check_run.name,
            bold("Url:"),
            check_run.url.as_ref().unwrap()
        ));

        for test_case in failing_test {
            for line in test_case {
                println!("{}", line);
            }
        }
    }

    Ok(())
}

pub async fn get_failing_test_files(
    repo: &Repository,
    failing_test_check_runs: Vec<&github::CheckRun>,
) -> Result<()> {
    let mut log_futures: FuturesUnordered<_> = failing_test_check_runs
        .iter()
        .map(|cr| github::get_job_logs(repo, cr))
        .collect();

    let mut failing_test_files: HashSet<String> = HashSet::new();

    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let logs = String::from_utf8_lossy(&bytes);
        failing_test_files.extend(extract_failing_test_files(&logs)?);
    }

    if failing_test_files.is_empty() {
        eprintln!("No failing test files found in log output");
        return Ok(());
    }

    for test_file in failing_test_files {
        println!("{}", test_file);
    }

    Ok(())
}
