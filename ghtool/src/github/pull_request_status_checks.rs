use eyre::Result;

pub fn extract_check_runs(pull_request: PullRequest) -> Result<Vec<CheckRun>> {
    let mut nodes = pull_request.status_check_rollup.nodes.unwrap();
    let pull_request_commit = nodes.remove(0);

    Ok(pull_request_commit
        .unwrap()
        .commit
        .status_check_rollup
        .unwrap()
        .contexts
        .nodes
        .unwrap()
        .into_iter()
        .map(|node| node.unwrap())
        .collect::<Vec<_>>()
        .into_iter()
        .filter_map(|x| match x {
            StatusCheckRollupContext::CheckRun(check_run) => Some(check_run),
            StatusCheckRollupContext::Unknown => None,
        })
        .collect::<Vec<_>>())
}

use cynic_github_schema as schema;

// https://github.com/obmarg/cynic/issues/713
#[derive(cynic::Scalar, Debug)]
#[cynic(graphql_type = "Int")]
pub struct BigInt(pub u64);

// Below is generated with https://generator.cynic-rs.dev using ./pull_request_status_checks.graphql,
// except database_id is changed from Option<i32> to Option<BigInt> manually.
// https://github.com/obmarg/cynic/issues/711
#[derive(cynic::QueryVariables, Debug)]
pub struct PullRequestStatusChecksVariables<'a> {
    pub id: &'a cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(graphql_type = "Query", variables = "PullRequestStatusChecksVariables")]
pub struct PullRequestStatusChecks {
    #[arguments(id: $id)]
    pub node: Option<Node>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub struct PullRequest {
    #[arguments(last: 1)]
    #[cynic(rename = "commits")]
    pub status_check_rollup: PullRequestCommitConnection,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub struct PullRequestCommitConnection {
    pub nodes: Option<Vec<Option<PullRequestCommit>>>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub struct PullRequestCommit {
    pub commit: Commit,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub struct Commit {
    pub status_check_rollup: Option<StatusCheckRollup>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub struct StatusCheckRollup {
    #[arguments(first: 100)]
    pub contexts: StatusCheckRollupContextConnection,
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub struct StatusCheckRollupContextConnection {
    pub nodes: Option<Vec<Option<StatusCheckRollupContext>>>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Debug)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub struct CheckRun {
    pub id: cynic::Id,
    pub url: Uri,
    pub external_id: Option<String>,
    pub name: String,
    pub status: CheckStatusState,
    pub conclusion: Option<CheckConclusionState>,
    pub started_at: Option<DateTime>,
    pub completed_at: Option<DateTime>,
    pub details_url: Option<Uri>,
    #[arguments(pullRequestId: $id)]
    pub is_required: bool,
    pub database_id: Option<BigInt>,
    pub __typename: String,
}

#[derive(cynic::InlineFragments, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub enum Node {
    PullRequest(PullRequest),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::InlineFragments, Debug)]
#[cynic(variables = "PullRequestStatusChecksVariables")]
pub enum StatusCheckRollupContext {
    CheckRun(CheckRun),
    #[cynic(fallback)]
    Unknown,
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq)]
pub enum CheckConclusionState {
    ActionRequired,
    Cancelled,
    Failure,
    Neutral,
    Skipped,
    Stale,
    StartupFailure,
    Success,
    TimedOut,
}

#[derive(cynic::Enum, Clone, Copy, Debug)]
pub enum CheckStatusState {
    Completed,
    InProgress,
    Pending,
    Queued,
    Requested,
    Waiting,
}

#[derive(cynic::Scalar, Debug, Clone)]
pub struct DateTime(pub String);

#[derive(cynic::Scalar, Debug, Clone)]
#[cynic(graphql_type = "URI")]
pub struct Uri(pub String);
