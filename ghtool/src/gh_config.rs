use eyre::Result;
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct SiteConfig {
    pub user: String,
    pub oauth_token: String,
    pub git_protocol: String,
}

pub type Config = HashMap<String, SiteConfig>;

pub struct GhConfig {
    config: HashMap<String, SiteConfig>,
}

impl GhConfig {
    pub fn new() -> Result<Self> {
        let config_path =
            get_config_path().ok_or_else(|| eyre::eyre!("Could not find config path"))?;

        let config_str = fs::read_to_string(&config_path).map_err(|e| {
            eyre::eyre!(
                "Error reading config from path {}: {}",
                config_path.to_string_lossy(),
                e
            )
        })?;

        let config: Config = serde_yaml::from_str(&config_str).unwrap();
        Ok(Self { config })
    }

    pub fn get_site_config(&self, hostname: &str) -> Result<&SiteConfig> {
        self.config.get(hostname).ok_or_else(|| {
            eyre::eyre!(
                r#"Could not find config for hostname {}
Have you run `gh auth login`?"#,
                hostname
            )
        })
    }
}

pub fn get_config_path() -> Option<PathBuf> {
    if let Some(mut home_path) = dirs::home_dir() {
        home_path.push(".config");
        home_path.push("gh");
        home_path.push("hosts.yml");
        return Some(home_path);
    }
    None
}
