use serde::{Deserialize, Serialize};

use super::{
    pull_request_for_branch::PullRequest,
    pull_request_status_checks::{CheckConclusionState, CheckRun},
};

#[derive(Debug)]
pub struct SimpleCheckRun {
    pub id: u64,
    pub name: String,
    pub conclusion: Option<CheckConclusionState>,
    pub url: Option<String>,
}

impl From<CheckRun> for SimpleCheckRun {
    fn from(check_run: CheckRun) -> Self {
        SimpleCheckRun {
            name: check_run.name,
            id: check_run.database_id.unwrap().0,
            conclusion: check_run.conclusion,
            url: check_run.details_url.map(|e| e.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplePullRequest {
    pub id: cynic::Id,
}

impl From<PullRequest> for SimplePullRequest {
    fn from(pull_request: PullRequest) -> Self {
        SimplePullRequest {
            id: pull_request.id,
        }
    }
}
