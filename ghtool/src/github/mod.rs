mod client;
mod pull_request_for_branch;
mod pull_request_status_checks;
mod types;

use bytes::Bytes;
use eyre::Result;
use std::time::Duration;

pub use client::GithubClient;
use futures::{stream::FuturesUnordered, Future};
use indicatif::{MultiProgress, ProgressBar};
pub use pull_request_status_checks::CheckConclusionState;
pub use types::*;

use crate::{git::Repository, spinner::make_spinner_style};

pub fn get_log_futures<'a>(
    client: &'a GithubClient,
    repo: &'a Repository,
    check_runs: &'a [SimpleCheckRun],
) -> FuturesUnordered<impl Future<Output = Result<Bytes>> + 'a> {
    let m = MultiProgress::new();
    let log_futures: FuturesUnordered<_> = check_runs
        .iter()
        .map(|cr| {
            let pb = m.add(ProgressBar::new_spinner());
            pb.enable_steady_tick(Duration::from_millis(100));
            pb.set_style(make_spinner_style());
            pb.set_message(format!("Fetching logs for check: {}", cr.name));

            async move {
                let result = client
                    .get_job_logs(&repo.owner, &repo.name, cr.id, &pb)
                    .await;
                pb.finish_and_clear();
                result
            }
        })
        .collect();

    log_futures
}
