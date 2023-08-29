use bytes::Bytes;
use eyre::Result;
use futures::future::try_join_all;
use indicatif::{MultiProgress, ProgressBar};
use std::collections::HashMap;
use std::time::Duration;

pub use self::auth_client::{AccessToken, AccessTokenResponse, CodeResponse, GithubAuthClient};
pub use self::client::{GithubApiError, GithubClient};
use crate::{git::Repository, spinner::make_spinner_style};

pub use current_user::CurrentUser;
pub use pull_request_status_checks::CheckConclusionState;
pub use types::*;
pub use wait_for_pr_checks::*;

mod auth_client;
mod client;
mod current_user;
mod pull_request_for_branch;
mod pull_request_status_checks;
mod types;
mod wait_for_pr_checks;

pub async fn fetch_check_run_logs(
    client: &GithubClient,
    repo: &Repository,
    check_runs: &[SimpleCheckRun],
) -> Result<HashMap<u64, Bytes>> {
    let m = MultiProgress::new();
    let log_futures: Vec<_> = check_runs
        .iter()
        .map(|cr| {
            let pb = m.add(ProgressBar::new_spinner());
            pb.enable_steady_tick(Duration::from_millis(100));
            pb.set_style(make_spinner_style());
            pb.set_message(format!("Fetching logs for check: {}", cr.name));

            let check_run_id = cr.id;
            async move {
                let result = client
                    .get_job_logs(&repo.owner, &repo.name, check_run_id, &pb)
                    .await;
                pb.finish_and_clear();
                result.map(|bytes| (check_run_id, bytes))
            }
        })
        .collect();

    let results = try_join_all(log_futures).await?;
    let log_map: HashMap<u64, Bytes> = results.into_iter().collect();
    Ok(log_map)
}
