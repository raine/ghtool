use std::collections::HashSet;

use eyre::Result;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use tracing::info;

use crate::github::get_log_futures;
use crate::github::CheckConclusionState;
use crate::github::GithubClient;
use crate::repo_config::TestConfig;
use crate::term::print_check_run_header;
use crate::{git::Repository, github, repo_config::RepoConfig, term::green};

mod jest;

use jest::*;

pub async fn tests(
    client: &GithubClient,
    repo: &Repository,
    branch: &str,
    repo_config: &RepoConfig,
    show_files_only: bool,
) -> Result<()> {
    let test_config = repo_config
        .test
        .as_ref()
        .ok_or_else(|| eyre::eyre!("No test config found in .ghtool.toml"))?;

    let pr = client
        .get_pr_for_branch_memoized(&repo.owner, &repo.name, branch)
        .await?;
    let check_runs = client.get_pr_status_checks(&pr.id).await?;
    let (test_check_runs, any_tests_in_progress) = filter_test_runs(check_runs, test_config);
    info!(?test_check_runs, "got test check runs");

    if test_check_runs.is_empty() {
        eprintln!(
            "No test jobs found matching the pattern /{}/",
            test_config.job_pattern
        );
    } else {
        process_failing_runs(
            client,
            repo,
            test_check_runs,
            any_tests_in_progress,
            show_files_only,
        )
        .await?;
    }

    Ok(())
}

async fn process_failing_runs(
    client: &GithubClient,
    repo: &Repository,
    test_check_runs: Vec<github::SimpleCheckRun>,
    any_tests_in_progress: bool,
    show_files_only: bool,
) -> Result<()> {
    let failing_test_check_runs: Vec<_> = test_check_runs
        .into_iter()
        .filter(|cr| cr.conclusion == Some(CheckConclusionState::Failure))
        .collect();

    if failing_test_check_runs.is_empty() {
        if any_tests_in_progress {
            eprintln!("⏳  Some test checks are in progress");
        } else {
            eprintln!("{}  All test checks are green", green("✓"));
        }
        return Ok(());
    }

    get_failing_tests(client, repo, failing_test_check_runs, show_files_only).await?;

    Ok(())
}

fn filter_test_runs(
    check_runs: Vec<github::SimpleCheckRun>,
    test_config: &TestConfig,
) -> (Vec<github::SimpleCheckRun>, bool) {
    let mut test_check_runs = Vec::new();
    let mut any_in_progress = false;

    for cr in check_runs {
        if test_config.job_pattern.is_match(&cr.name) {
            if cr.conclusion.is_none() {
                any_in_progress = true;
            }
            test_check_runs.push(cr);
        }
    }

    (test_check_runs, any_in_progress)
}

async fn get_failing_tests(
    client: &GithubClient,
    repo: &Repository,
    failing_test_check_runs: Vec<github::SimpleCheckRun>,
    show_files_only: bool,
) -> Result<()> {
    let mut log_futures: FuturesUnordered<_> =
        get_log_futures(client, repo, &failing_test_check_runs);

    let mut failing_tests = Vec::new();
    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let logs = String::from_utf8_lossy(&bytes);
        let failing_tests_inner = JestLogParser::parse(&logs)?;
        failing_tests.push(failing_tests_inner);
    }

    if failing_tests.iter().all(|s| s.is_empty()) {
        eprintln!("No failing tests found in log output");
        return Ok(());
    }

    if show_files_only {
        print_failed_test_files(failing_tests);
    } else {
        print_failed_tests(&failing_test_check_runs, failing_tests);
    }

    Ok(())
}

fn print_failed_test_files(failing_tests: Vec<Vec<JestPath>>) {
    let files: HashSet<String> = failing_tests
        .into_iter()
        .flat_map(|jest_paths| jest_paths.into_iter().map(|jest_path| jest_path.path))
        .collect();

    for file in files {
        println!("{}", file);
    }
}

fn print_failed_tests(
    failing_test_check_runs: &[github::SimpleCheckRun],
    failing_tests: Vec<Vec<JestPath>>,
) {
    failing_test_check_runs
        .iter()
        .zip(failing_tests)
        .for_each(|(check_run, failing_test)| {
            print_check_run_header(check_run);

            failing_test
                .into_iter()
                .flat_map(|jest_path| jest_path.lines)
                .for_each(|line| println!("{}", line));
        });
}
