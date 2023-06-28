use async_trait::async_trait;
use eyre::Result;
use regex::Regex;

use crate::repo_config::RepoConfig;
use crate::repo_config::TypecheckConfig;

use self::tsc::TscLogParser;

use super::CheckError;
use super::Command;
use super::ConfigPattern;

mod tsc;

impl ConfigPattern for TypecheckConfig {
    fn job_pattern(&self) -> &Regex {
        &self.job_pattern
    }
}

pub struct TypecheckCommand {
    config: TypecheckConfig,
}

impl TypecheckCommand {
    pub fn from_repo_config(repo_config: RepoConfig) -> Result<Self> {
        let typecheck_config = repo_config
            .typecheck
            .ok_or_else(|| eyre::eyre!("Error: no typecheck section found in .ghtool.toml"))?;

        Ok(Self {
            config: typecheck_config,
        })
    }
}

#[async_trait]
impl Command for TypecheckCommand {
    type ConfigType = TypecheckConfig;

    fn name(&self) -> &'static str {
        "typecheck"
    }

    fn check_error_plural(&self) -> &'static str {
        "type errors"
    }

    fn config(&self) -> &Self::ConfigType {
        &self.config
    }

    fn parse_log(&self, log: &str) -> Result<Vec<CheckError>> {
        TscLogParser::parse(log)
    }
}
