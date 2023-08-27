use cynic::Id;
use eyre::Result;
use indicatif::{MultiProgress, ProgressBar};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::spinner::{make_job_completed_spinner, make_job_failed_spinner, make_job_spinner};
use crate::term::{bold, exit_with_error};

use super::{CheckConclusionState, GithubClient, SimpleCheckRun};

const POLL_INTERVAL: Duration = Duration::from_secs(10);

pub async fn wait_for_pr_checks(
    client: &GithubClient,
    pull_request_id: Id,
) -> Result<Vec<SimpleCheckRun>> {
    let m = MultiProgress::new();
    let spinners = Arc::new(Mutex::new(HashMap::new()));

    let initial_check_runs = client.get_pr_status_checks(&pull_request_id, true).await?;
    let all_completed = initial_check_runs
        .iter()
        .all(|check_run| check_run.completed_at.map_or(false, |_| true));

    if all_completed {
        return Ok(initial_check_runs);
    }

    let max_check_name_length = initial_check_runs
        .iter()
        .map(|check_run| check_run.name.len())
        .max()
        .unwrap_or(0);

    for check_run in initial_check_runs.iter() {
        get_or_insert_spinner(&spinners, check_run, &m, max_check_name_length).await;
    }

    tokio::time::sleep(POLL_INTERVAL).await;

    let check_runs = loop {
        match client.get_pr_status_checks(&pull_request_id, false).await {
            Ok(check_runs) => {
                if process_check_runs(&m, &check_runs, &spinners).await {
                    break check_runs;
                }
            }
            Err(e) => exit_with_error(e),
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    };

    Ok(check_runs)
}

async fn process_check_runs(
    m: &MultiProgress,
    check_runs: &[SimpleCheckRun],
    spinners: &Arc<Mutex<HashMap<u64, ProgressBar>>>,
) -> bool {
    let mut all_completed = true;
    let max_check_name_length = check_runs
        .iter()
        .map(|check_run| check_run.name.len())
        .max()
        .unwrap_or(0);

    for check_run in check_runs.iter() {
        let pb = get_or_insert_spinner(spinners, check_run, m, max_check_name_length).await;
        if check_run.completed_at.is_some() {
            update_spinner_on_completion(&pb, check_run);
        } else {
            all_completed = false;
        }
    }

    all_completed
}

async fn get_or_insert_spinner(
    spinners: &Arc<Mutex<HashMap<u64, ProgressBar>>>,
    check_run: &SimpleCheckRun,
    m: &MultiProgress,
    max_check_name_length: usize,
) -> ProgressBar {
    let mut spinners = spinners.lock().await;
    spinners
        .entry(check_run.id)
        .or_insert_with(|| add_spinner(check_run, m, max_check_name_length))
        .clone()
}

fn add_spinner(
    check_run: &SimpleCheckRun,
    m: &MultiProgress,
    max_check_name_length: usize,
) -> ProgressBar {
    let mut pb = ProgressBar::new_spinner();

    if let Some(elapsed) = check_run.elapsed() {
        pb = pb.with_elapsed(elapsed);
    }

    // Pad the name with max_check_name_length so that elapsed durations are aligned
    let padded_name = format!(
        "{:<width$}",
        check_run.name,
        width = max_check_name_length + 1
    );
    m.add(pb.clone());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_style(make_job_spinner());
    pb.set_message(format!("Waiting: {}", bold(&padded_name)));
    pb
}

fn update_spinner_on_completion(pb: &ProgressBar, check_run: &SimpleCheckRun) {
    let (style, prefix, message) = match check_run.conclusion {
        Some(CheckConclusionState::Success) => (
            make_job_completed_spinner(),
            "✓",
            format!("Check {} completed in", bold(&check_run.name)),
        ),
        Some(CheckConclusionState::Failure) => (
            make_job_failed_spinner(),
            "X",
            format!("Check {} failed in", bold(&check_run.name)),
        ),
        _ => (
            make_job_spinner(),
            "-",
            format!("Check {} completed in", bold(&check_run.name)),
        ),
    };

    pb.set_style(style);
    pb.set_prefix(prefix);
    pb.finish_with_message(message);
}
