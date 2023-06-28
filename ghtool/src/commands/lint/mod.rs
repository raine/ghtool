use async_trait::async_trait;
use eyre::Result;
use regex::Regex;

use crate::repo_config::LintConfig;
use crate::repo_config::RepoConfig;

use self::eslint::EslintLogParser;

use super::CheckError;
use super::Command;
use super::ConfigPattern;

mod eslint;

impl ConfigPattern for LintConfig {
    fn job_pattern(&self) -> &Regex {
        &self.job_pattern
    }
}

pub struct LintCommand {
    config: LintConfig,
}

impl LintCommand {
    pub fn from_repo_config(repo_config: RepoConfig) -> Result<Self> {
        let lint_config = repo_config
            .lint
            .ok_or_else(|| eyre::eyre!("Error: no lint section found in .ghtool.toml"))?;

        Ok(Self {
            config: lint_config,
        })
    }
}

#[async_trait]
impl Command for LintCommand {
    type ConfigType = LintConfig;

    fn name(&self) -> &'static str {
        "lint"
    }

    fn check_error_plural(&self) -> &'static str {
        "lint issues"
    }

    fn config(&self) -> &Self::ConfigType {
        &self.config
    }

    fn parse_log(&self, log: &str) -> Result<Vec<CheckError>> {
        Ok(EslintLogParser::parse(log))
    }
}
