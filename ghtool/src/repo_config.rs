use eyre::Result;
use serde::{Deserialize, Deserializer};
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
pub struct RepoConfig {
    pub test: Option<TestConfig>,
    pub lint: Option<LintConfig>,
}

#[derive(Debug, Deserialize)]
pub struct TestConfig {
    /// Regex pattern that is used to match against test job names
    #[serde(deserialize_with = "deserialize_regex")]
    pub job_pattern: regex::Regex,

    #[serde(deserialize_with = "TestRunner::deserialize")]
    pub runner: TestRunner,
}

#[derive(Debug, Deserialize)]
pub struct LintConfig {
    #[serde(deserialize_with = "deserialize_regex")]
    /// Regex pattern that is used to match against lint job names
    pub job_pattern: regex::Regex,

    #[serde(deserialize_with = "LintTool::deserialize")]
    pub tool: LintTool,
}

#[derive(Debug)]
pub enum TestRunner {
    Jest,
}

impl<'de> Deserialize<'de> for TestRunner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "jest" => Ok(TestRunner::Jest),
            _ => Err(serde::de::Error::custom(format!(
                "invalid test runner: {}",
                s
            ))),
        }
    }
}

#[derive(Debug)]
pub enum LintTool {
    Eslint,
}

impl<'de> Deserialize<'de> for LintTool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "eslint" => Ok(LintTool::Eslint),
            _ => Err(serde::de::Error::custom(format!(
                "invalid lint tool: {}",
                s
            ))),
        }
    }
}

fn deserialize_regex<'de, D>(deserializer: D) -> Result<regex::Regex, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    regex::Regex::new(&s).map_err(serde::de::Error::custom)
}

pub fn read_repo_config_from_path(config_path: &Path) -> Result<RepoConfig> {
    let config_str = fs::read_to_string(config_path).map_err(|e| {
        eyre::eyre!(
            "Error reading config from path {}: {}",
            config_path.to_string_lossy(),
            e
        )
    })?;

    let config: RepoConfig = toml::from_str(&config_str)?;
    Ok(config)
}

pub fn read_repo_config(repo_path: &Path) -> Result<RepoConfig> {
    let config_path = repo_path.join(".ghtool.toml");
    read_repo_config_from_path(&config_path)
}
