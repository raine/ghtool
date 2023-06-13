use std::collections::HashSet;

use eyre::Result;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use github::graphql::CheckConclusionState;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{
    git::Repository,
    github,
    repo_config::RepoConfig,
    term::{bold, exit_with_message, green, print_header},
};

const TIMESTAMP_PATTERN: &str = r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)";

lazy_static! {
    /// Regex to match a timestamp at the start of a line including the whitespace after it
    static ref TIMESTAMP: Regex = Regex::new(TIMESTAMP_PATTERN).unwrap();

    /// Regex to match a failing jest test. The path needs to contain at least one slash.
    /// Example: 2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent.test.tsx
    static ref JEST_FAIL_LINE: Regex = Regex::new(&format!(
        r"{TIMESTAMP_PATTERN}\s+(?P<fail>FAIL)\s+(?P<path>[a-zA-Z0-9._-]*/[a-zA-Z0-9./_-]*)",
    ))
    .unwrap();
}

pub async fn failing_tests(
    repo: &Repository,
    branch: &str,
    repo_config: &RepoConfig,
    show_files_only: bool,
) -> Result<()> {
    let pr = github::get_pr_for_branch(repo, branch).await?;
    let pr_id = pr.node_id.expect("pr node_id is missing");
    let check_runs = github::get_pr_status_checks(&pr_id).await?;
    let (test_check_runs, any_in_progress) = filter_test_runs(&check_runs, repo_config);

    if test_check_runs.is_empty() {
        println!(
            "No test jobs found matching the pattern /{}/",
            repo_config.test_job_pattern
        );
    } else if any_in_progress {
        println!("No failed test runs, but some checks are still pending");
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

fn extract_failing_tests(logs: &str) -> Result<Vec<Vec<String>>, eyre::Error> {
    let mut fail_start_col = 0;
    let mut in_test_case = false;
    let mut current_fail_lines = Vec::new();
    let mut failing_tests_inner = Vec::new();

    // Collect failing tests from the logs by reading lines from a line that matches JEST_FAIL
    // until there is a line where there is something else than whitespace in the same column
    // as the FAIL match.
    //
    // 2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent.test.tsx
    // 2021-05-04T18:24:29.000Z   ● Test suite failed to run
    // 2021-05-04T18:24:29.000Z     TypeError: Cannot read property 'foo' of undefined
    // 2021-05-04T18:24:29.000Z
    // 2021-05-04T18:24:29.000Z       1 | import React from 'react';
    // 2021-05-04T18:24:29.000Z PASS src/components/MyComponent/MyComponent.test.tsx
    for full_line in logs.lines() {
        let line_no_ansi = String::from_utf8(strip_ansi_escapes::strip(full_line.as_bytes())?)?;
        let line_no_timestamp = TIMESTAMP.replace(full_line, "");

        if let Some(caps) = JEST_FAIL_LINE.captures(&line_no_ansi) {
            fail_start_col = caps.name("fail").unwrap().start();
            current_fail_lines.push(line_no_timestamp.to_string());
            in_test_case = true;
        } else if in_test_case {
            if line_no_ansi.len() > fail_start_col
                && line_no_ansi.chars().nth(fail_start_col) != Some(' ')
            {
                failing_tests_inner.push(current_fail_lines);
                current_fail_lines = Vec::new();
                in_test_case = false;
            } else {
                current_fail_lines.push(line_no_timestamp.to_string());
            }
        }
    }

    Ok(failing_tests_inner)
}

pub async fn get_failing_tests(
    repo: &Repository,
    failing_test_check_runs: Vec<&github::CheckRun>,
) -> Result<()> {
    let mut log_futures: FuturesUnordered<_> = failing_test_check_runs
        .iter()
        .map(|cr| github::get_job_logs(repo, cr.id))
        .collect();

    let mut failing_tests = Vec::new();

    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let logs = String::from_utf8_lossy(&bytes);
        let failing_tests_inner = extract_failing_tests(&logs)?;
        failing_tests.push(failing_tests_inner);
    }

    if failing_tests.iter().all(|s| s.is_empty()) {
        println!("No failing tests found in log output");
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
        .map(|cr| github::get_job_logs(repo, cr.id))
        .collect();

    let mut failing_test_files: HashSet<String> = HashSet::new();

    while let Some(result) = log_futures.next().await {
        let bytes = result.map_err(|_| eyre::eyre!("Error when getting job logs"))?;
        let logs = String::from_utf8_lossy(&bytes);
        for full_line in logs.lines() {
            let line_no_ansi = String::from_utf8(
                strip_ansi_escapes::strip(full_line.as_bytes())
                    .map_err(|_| eyre::eyre!("Error when stripping ansi escapes"))?,
            )?;
            if let Some(caps) = JEST_FAIL_LINE.captures(&line_no_ansi) {
                failing_test_files.insert(caps["path"].to_string());
            }
        }
    }

    if failing_test_files.is_empty() {
        println!("No failing test files found in log output");
        return Ok(());
    }

    for test_file in failing_test_files {
        println!("{}", test_file);
    }

    Ok(())
}
