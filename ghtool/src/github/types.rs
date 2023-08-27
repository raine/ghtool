use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    pull_request_for_branch::PullRequest,
    pull_request_status_checks::{CheckConclusionState, CheckRun},
};

#[derive(Debug, Clone)]
pub struct SimpleCheckRun {
    pub id: u64,
    pub name: String,
    pub conclusion: Option<CheckConclusionState>,
    pub url: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl SimpleCheckRun {
    pub fn elapsed(&self) -> Option<std::time::Duration> {
        self.started_at.map(|started_at| {
            Utc::now()
                .signed_duration_since(started_at)
                .to_std()
                .unwrap()
        })
    }
}

impl From<CheckRun> for SimpleCheckRun {
    fn from(check_run: CheckRun) -> Self {
        SimpleCheckRun {
            name: check_run.name,
            id: check_run.database_id.unwrap().0,
            conclusion: check_run.conclusion,
            url: check_run.details_url.map(|e| e.0),
            started_at: check_run.started_at.map(|e| {
                DateTime::parse_from_rfc3339(&e.0)
                    .expect("Failed to parse date")
                    .with_timezone(&chrono::Utc)
            }),
            completed_at: check_run.completed_at.map(|e| {
                DateTime::parse_from_rfc3339(&e.0)
                    .expect("Failed to parse date")
                    .with_timezone(&chrono::Utc)
            }),
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
