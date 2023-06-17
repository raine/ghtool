use cynic::QueryBuilder;
use eyre::Result;
use reqwest::header::HeaderMap;
use tracing::info;

use crate::{
    cache,
    github::{
        pull_request_for_branch::{
            extract_pull_request, PullRequestForBranch, PullRequestForBranchVariables,
        },
        pull_request_status_checks::{
            extract_check_runs, Node, PullRequestStatusChecks, PullRequestStatusChecksVariables,
        },
    },
};

use super::{types::SimpleCheckRun, SimplePullRequest};

pub struct GithubClient {
    client: reqwest::Client,
}

const GITHUB_BASE_URI: &str = "https://api.github.com";

impl GithubClient {
    pub fn new(oauth_token: String) -> Result<Self> {
        let client = Self::make_base_client(&oauth_token)?;
        Ok(Self { client })
    }

    fn make_headers(oauth_token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("token {}", oauth_token).parse().unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github.v3+json".parse().unwrap(),
        );
        headers
    }

    fn make_base_client(oauth_token: &str) -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .user_agent("ghtool")
            .default_headers(Self::make_headers(oauth_token))
            .build()
            .map_err(|e| eyre::eyre!("Failed to build client: {}", e))
    }

    pub async fn run_graphql_query<T, K>(&self, operation: cynic::Operation<T, K>) -> Result<T>
    where
        T: serde::de::DeserializeOwned + 'static,
        K: serde::Serialize,
    {
        use cynic::http::ReqwestExt;
        let response = self
            .client
            .post("https://api.github.com/graphql")
            .run_graphql(operation)
            .await?;

        response
            .data
            .ok_or_else(|| eyre::eyre!("no data in response"))
    }

    pub async fn get_pr_for_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<SimplePullRequest> {
        info!(?owner, ?repo, ?branch, "getting pr for branch");
        let query = PullRequestForBranch::build(PullRequestForBranchVariables {
            head_ref_name: branch,
            owner,
            repo,
            states: None,
        });

        let pr_for_branch = self.run_graphql_query(query).await?;
        info!(?pr_for_branch, "got pr");
        let pr = extract_pull_request(pr_for_branch);
        Ok(pr)
    }

    pub async fn get_pr_for_branch_memoized(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<SimplePullRequest> {
        let key = format!("pr_for_branch_{}_{}", repo, branch);
        cache::memoize(key, || self.get_pr_for_branch(owner, repo, branch)).await
    }

    pub async fn get_pr_status_checks(&self, id: &cynic::Id) -> Result<Vec<SimpleCheckRun>> {
        info!(?id, "getting checks for pr");
        let query = PullRequestStatusChecks::build(PullRequestStatusChecksVariables { id });
        let pr_checks = self.run_graphql_query(query).await?;
        match pr_checks.node {
            Some(Node::PullRequest(pull_request)) => {
                let check_runs = extract_check_runs(pull_request)?;
                Ok(check_runs.into_iter().map(SimpleCheckRun::from).collect()) // convert check runs
            }
            Some(Node::Unknown) => eyre::bail!("Unknown node type"),
            None => eyre::bail!("No node in response"),
        }
    }

    pub async fn get_job_logs(&self, owner: &str, repo: &str, job_id: u64) -> Result<bytes::Bytes> {
        info!(?owner, ?repo, ?job_id, "getting job logs");
        let url = format!("{GITHUB_BASE_URI}/repos/{owner}/{repo}/actions/jobs/{job_id}/logs",);

        self.client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await
            .map_err(|e| eyre::eyre!("Failed to get bytes: {}", e))
    }
}
