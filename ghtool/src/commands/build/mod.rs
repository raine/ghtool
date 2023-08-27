use eyre::Result;
use regex::Regex;

use crate::repo_config::BuildConfig;
use crate::repo_config::RepoConfig;

use self::tsc::TscLogParser;

use super::CheckError;
use super::Command;
use super::ConfigPattern;

mod tsc;

impl ConfigPattern for BuildConfig {
    fn job_pattern(&self) -> &Regex {
        &self.job_pattern
    }
}

#[derive(Clone)]
pub struct BuildCommand {
    config: BuildConfig,
}

impl BuildCommand {
    pub fn from_repo_config(repo_config: &RepoConfig) -> Result<Self> {
        let build_config = repo_config
            .build
            .clone()
            .ok_or_else(|| eyre::eyre!("Error: no build section found in .ghtool.toml"))?;

        Ok(Self {
            config: build_config,
        })
    }
}

impl Command for BuildCommand {
    fn name(&self) -> &'static str {
        "build"
    }

    fn check_error_plural(&self) -> &'static str {
        "build errors"
    }

    fn config(&self) -> &dyn ConfigPattern {
        &self.config
    }

    fn parse_log(&self, log: &str) -> Result<Vec<CheckError>> {
        TscLogParser::parse(log)
    }
}
