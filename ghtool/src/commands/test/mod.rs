use eyre::Result;
use regex::Regex;

use crate::repo_config::RepoConfig;
use crate::repo_config::TestConfig;

pub mod jest;

use jest::*;

use super::command::CheckError;
use super::command::Command;
use super::command::ConfigPattern;

impl ConfigPattern for TestConfig {
    fn job_pattern(&self) -> &Regex {
        &self.job_pattern
    }
}

#[derive(Clone)]
pub struct TestCommand {
    config: TestConfig,
}

impl TestCommand {
    pub fn from_repo_config(repo_config: &RepoConfig) -> Result<Self> {
        let test_config = repo_config
            .test
            .clone()
            .ok_or_else(|| eyre::eyre!("Error: no test section found in .ghtool.toml"))?;

        Ok(Self {
            config: test_config,
        })
    }
}

impl Command for TestCommand {
    fn name(&self) -> &'static str {
        "test"
    }

    fn check_error_plural(&self) -> &'static str {
        "test errors"
    }

    fn config(&self) -> &dyn ConfigPattern {
        &self.config
    }

    fn parse_log(&self, log: &str) -> Result<Vec<CheckError>> {
        JestLogParser::parse(log)
    }
}
