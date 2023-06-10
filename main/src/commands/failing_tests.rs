use eyre::Result;
use github::graphql::CheckConclusionState;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    git::Repository,
    github,
    repo_config::RepoConfig,
    term::{bold, exit_with_message, green, print_header},
};

lazy_static! {
    /// Regex to match a timestamp at the start of a line including the whitespace after it
    static ref TIMESTAMP: Regex = Regex::new(
        r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)\s+"
    ).unwrap();

    /// Regex to match a failing jest test. The path needs to contain at least one slash.
    /// Example: 2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent.test.tsx
    static ref JEST_FAIL: Regex = Regex::new(
        r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)\s+FAIL\s+(?P<path>[a-zA-Z0-9._-]*/[a-zA-Z0-9./_-]*)"
    ).unwrap();

    /// Regex to match the start of jest summary
    static ref JEST_SUMMARY_START: Regex = Regex::new(
        r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)\s+Summary of all failing tests$"
    ).unwrap();

    /// Regex to match the end of jest summary
    static ref JEST_SUMMARY_END: Regex = Regex::new(
        r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)\s+Ran all test suites"
    ).unwrap();
}

pub async fn failing_tests(
    repo: &Repository,
    branch: &str,
    repo_config: &RepoConfig,
    summary: bool,
) -> Result<()> {
    let pr = github::get_pr_for_branch(repo, branch).await?;
    let pr_id = pr.node_id.expect("pr node_id is missing");
    let check_runs = github::get_pr_status_checks(&pr_id).await?;

    let test_check_runs = check_runs
        .iter()
        .filter(|cr| repo_config.test_job_pattern.is_match(&cr.name))
        .collect::<Vec<_>>();

    let any_test_check_runs_in_progress = test_check_runs.iter().any(|cr| cr.conclusion.is_none());

    if test_check_runs.is_empty() {
        exit_with_message(
            1,
            &format!(
                "No test jobs found matching the pattern /{}/",
                repo_config.test_job_pattern
            ),
        );
    }

    let failing_test_check_runs = test_check_runs
        .into_iter()
        .filter(|cr| cr.conclusion == Some(CheckConclusionState::Failure))
        .collect::<Vec<_>>();

    if failing_test_check_runs.is_empty() && any_test_check_runs_in_progress {
        exit_with_message(0, "No failed test runs, but some checks are still pending");
    }

    if failing_test_check_runs.is_empty() && !any_test_check_runs_in_progress {
        exit_with_message(0, &format!("{}  All test checks are green", green("✓")));
    }

    if summary {
        get_summary(repo, failing_test_check_runs).await?;
    } else {
        get_failing_tests(repo, failing_test_check_runs).await?;
    }

    Ok(())
}

pub async fn get_summary(
    repo: &Repository,
    failing_test_check_runs: Vec<&github::CheckRun>,
) -> Result<()> {
    let log_futures = failing_test_check_runs.iter().map(|cr| async {
        let bytes = github::get_job_logs(repo, cr.id).await?;
        let logs = String::from_utf8_lossy(&bytes);
        let mut in_summary = false;
        let mut lines = vec![];
        for line in logs.lines() {
            let without_ansi_bytes = strip_ansi_escapes::strip(line)?;
            let without_ansi = String::from_utf8(without_ansi_bytes.to_vec())?;

            if JEST_SUMMARY_START.is_match(&without_ansi) {
                in_summary = true;
            } else if JEST_SUMMARY_END.is_match(&without_ansi) {
                in_summary = false;
            } else if in_summary {
                lines.push(TIMESTAMP.replace(line, "").to_string());
            }
        }

        Ok::<_, eyre::Error>(lines)
    });

    let summaries = futures::future::join_all(log_futures).await;
    if summaries.iter().all(|s| s.as_ref().unwrap().is_empty()) {
        println!("No failing test summaries found in log output");
        std::process::exit(0);
    }

    for (check_run, summary) in failing_test_check_runs.iter().zip(summaries) {
        print_header(&format!(
            "{} {}\n{} {}",
            bold("Job:"),
            check_run.name,
            bold("Url:"),
            check_run.url.as_ref().unwrap()
        ));

        for line in summary? {
            println!("{}", line);
        }
        println!();
    }

    Ok(())
}

pub async fn get_failing_tests(
    repo: &Repository,
    failing_test_check_runs: Vec<&github::CheckRun>,
) -> Result<()> {
    let log_futures = failing_test_check_runs.iter().map(|cr| async {
        let bytes = github::get_job_logs(repo, cr.id).await?;
        let logs = String::from_utf8_lossy(&bytes);
        let mut failing_test_files = Vec::new();
        for cap in JEST_FAIL.captures_iter(&logs) {
            failing_test_files.push(cap["path"].to_string());
        }
        Ok::<_, eyre::Error>(failing_test_files)
    });

    let results = futures::future::join_all(log_futures).await;
    let mut failing_test_files = Vec::new();
    for r in results {
        failing_test_files.extend(r?);
    }

    if failing_test_files.is_empty() {
        exit_with_message(0, "No failing test files found in log output");
    }

    failing_test_files.sort();
    failing_test_files.dedup();

    for test_file in failing_test_files {
        println!("{}", test_file);
    }

    Ok(())
}
