use eyre::{Result, WrapErr};
use serde::{Deserialize, Deserializer};
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
pub struct RepoConfig {
    pub test: Option<TestConfig>,
    pub lint: Option<LintConfig>,
    pub typecheck: Option<TypecheckConfig>,
}

#[derive(Debug, Deserialize)]
pub struct TestConfig {
    #[serde(deserialize_with = "deserialize_regex")]
    pub job_pattern: regex::Regex,
    pub tool: TestRunner,
}

#[derive(Debug, Deserialize)]
pub struct LintConfig {
    #[serde(deserialize_with = "deserialize_regex")]
    pub job_pattern: regex::Regex,
    pub tool: LintTool,
}

#[derive(Debug, Deserialize)]
pub struct TypecheckConfig {
    #[serde(deserialize_with = "deserialize_regex")]
    pub job_pattern: regex::Regex,
    pub tool: TypecheckTool,
}

#[derive(Debug)]
pub enum TestRunner {
    Jest,
}

#[derive(Debug)]
pub enum LintTool {
    Eslint,
}

#[derive(Debug)]
pub enum TypecheckTool {
    Tsc,
}

fn deserialize_tool<'de, D, T>(
    deserializer: D,
    valid_tool: &'static str,
    tool: T,
    tool_name: &str,
) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.eq_ignore_ascii_case(valid_tool) {
        Ok(tool)
    } else {
        Err(serde::de::Error::custom(format!(
            "invalid {}: {}",
            tool_name, s
        )))
    }
}

impl<'de> Deserialize<'de> for TestRunner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tool(deserializer, "jest", TestRunner::Jest, "test runner")
    }
}

impl<'de> Deserialize<'de> for LintTool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tool(deserializer, "eslint", LintTool::Eslint, "lint tool")
    }
}

impl<'de> Deserialize<'de> for TypecheckTool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserialize_tool(deserializer, "tsc", TypecheckTool::Tsc, "typecheck tool")
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
    let config_str = fs::read_to_string(config_path).wrap_err_with(|| {
        format!(
            "Error reading config from path {}",
            config_path.to_string_lossy()
        )
    })?;
    let config: RepoConfig = toml::from_str(&config_str)?;
    Ok(config)
}

pub fn read_repo_config(repo_path: &Path) -> Result<RepoConfig> {
    let config_path = repo_path.join(".ghtool.toml");
    read_repo_config_from_path(&config_path)
}
