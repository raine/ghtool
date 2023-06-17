use cynic_github_schema as schema;
use serde::Serialize;

use super::SimplePullRequest;

pub fn extract_pull_request(pr_for_branch: PullRequestForBranch) -> SimplePullRequest {
    pr_for_branch
        .repository
        .expect("no repository in response")
        .pull_requests
        .nodes
        .expect("no nodes in response")
        .into_iter()
        .next()
        .flatten()
        .map(SimplePullRequest::from)
        .expect("no pull requests in response")
}

// Below is generated with https://generator.cynic-rs.dev using ./pull_request_for_branch.graphql,
#[derive(cynic::QueryVariables, Debug)]
pub struct PullRequestForBranchVariables<'a> {
    pub head_ref_name: &'a str,
    pub owner: &'a str,
    pub repo: &'a str,
    pub states: Option<Vec<PullRequestState>>,
}

#[derive(cynic::QueryFragment, Debug, Serialize)]
pub struct User {
    pub name: Option<String>,
    pub id: cynic::Id,
    pub login: String,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PullRequestForBranchVariables")]
pub struct PullRequestForBranch {
    #[arguments(owner: $owner, name: $repo)]
    pub repository: Option<Repository>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PullRequestForBranchVariables")]
pub struct Repository {
    #[arguments(headRefName: $head_ref_name, states: $states, first: 30, orderBy: { direction: "DESC", field: "CREATED_AT" })]
    pub pull_requests: PullRequestConnection,
    pub default_branch_ref: Option<Ref>,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct Ref {
    pub name: String,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct PullRequestConnection {
    pub nodes: Option<Vec<Option<PullRequest>>>,
}

#[derive(cynic::QueryFragment, Debug, Serialize)]
pub struct PullRequest {
    pub number: i32,
    pub head_ref_name: String,
    pub id: cynic::Id,
    pub state: PullRequestState,
    pub base_ref_name: String,
    pub is_cross_repository: bool,
    pub head_repository_owner: Option<RepositoryOwner>,
}

#[derive(cynic::InlineFragments, Debug, Serialize)]
pub enum RepositoryOwner {
    User(User),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum PullRequestState {
    Closed,
    Merged,
    Open,
}
