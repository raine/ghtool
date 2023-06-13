use eyre::Result;
use http::Uri;
use hyper::body;
use octocrab::models::pulls::PullRequest;
use tracing::info;

use crate::{cache, git::Repository};

pub mod graphql;
mod pull_request_status_checks;

pub use pull_request_status_checks::*;

pub async fn get_pr_for_branch(repo: &Repository, branch: &str) -> Result<PullRequest> {
    info!(?branch, ?repo, "getting pr for branch");

    let client = octocrab::instance();
    let page = client
        .pulls(&repo.owner, &repo.name)
        .list()
        .state(octocrab::params::State::Open)
        .head(&format!("{}:{}", repo.owner, branch))
        .send()
        .await?;
    let pr = page
        .items
        .into_iter()
        .next()
        .ok_or_else(|| eyre::eyre!("No PR found for branch {}", branch))?;
    Ok(pr)
}

pub async fn get_pr_for_branch_memoized(repo: &Repository, branch: &str) -> Result<PullRequest> {
    let key = format!("pr_for_branch_{}_{}", repo, branch);
    cache::memoize(key, || get_pr_for_branch(repo, branch)).await
}

pub async fn get_job_logs(repo: &Repository, check_run: &CheckRun) -> Result<hyper::body::Bytes> {
    info!(?check_run, "getting job logs");

    let client = octocrab::instance();
    let route = format!(
        "/repos/{owner}/{repo}/actions/jobs/{job_id}/logs",
        owner = repo.owner,
        repo = repo.name,
        job_id = check_run.id,
    );
    let uri = Uri::builder().path_and_query(route).build()?;
    let data_response = client
        .follow_location_to_data(client._get(uri).await?)
        .await?;
    let body = data_response.into_body();
    body::to_bytes(body).await.map_err(Into::into)
}
