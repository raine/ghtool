use eyre::Result;

use crate::github::graphql as gql;

pub struct CheckRun {
    pub name: String,
    pub id: u64,
    pub conclusion: Option<gql::CheckConclusionState>,
    pub url: Option<String>,
}

pub async fn get_pr_status_checks(pr_id: &str) -> Result<Vec<CheckRun>> {
    use cynic::QueryBuilder;

    let client = octocrab::instance();
    let id = &cynic::Id::new(pr_id);
    let query = gql::PullRequestStatusChecks::build(gql::PullRequestStatusChecksVariables { id });
    let response: cynic::GraphQlResponse<gql::PullRequestStatusChecks> =
        client.graphql(&query).await?;

    Ok(match response.data.unwrap().node.unwrap() {
        gql::Node::PullRequest(pull_request) => extract_check_runs(pull_request)?,
        gql::Node::Unknown => panic!("Unknown node type"),
    })
}

fn extract_check_runs(pull_request: gql::PullRequest) -> Result<Vec<CheckRun>> {
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
            gql::StatusCheckRollupContext::CheckRun(check_run) => Some(check_run),
            gql::StatusCheckRollupContext::Unknown => None,
        })
        .map(|check_run| CheckRun {
            name: check_run.name,
            id: check_run.database_id.unwrap().0,
            conclusion: check_run.conclusion,
            url: check_run.details_url.map(|e| e.0),
        })
        .collect::<Vec<_>>())
}
