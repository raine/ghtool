use std::{
    env,
    path::{Path, PathBuf},
};

use clap::Parser;
use eyre::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{
    cli::Cli,
    gh_config::GhConfig,
    git::{Git, Repository},
    repo_config::{read_repo_config, RepoConfig},
};

pub fn setup() -> Result<(Cli, Repository, String, RepoConfig)> {
    setup_env()?;
    let cli = Cli::parse();

    let (gh_config, repo_path, repo_config) = setup_configs()?;
    let (repo, branch) = get_git_info(&repo_path)?;
    setup_octocrab(&gh_config, &repo)?;

    Ok((cli, repo, branch, repo_config))
}

fn setup_env() -> Result<()> {
    color_eyre::install()?;

    // // Default to info log level
    // if std::env::var("RUST_LOG").is_err() {
    //     std::env::set_var("RUST_LOG", "info");
    // }

    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1");
    }

    tracing_subscriber::fmt()
        .without_time()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    Ok(())
}

fn setup_octocrab(gh_config: &GhConfig, repo: &Repository) -> Result<()> {
    let site_config = gh_config.get_site_config(&repo.hostname)?;
    let client = octocrab::Octocrab::builder()
        .personal_token(site_config.oauth_token.to_string())
        .build()?;
    octocrab::initialise(client);
    Ok(())
}

fn setup_configs() -> Result<(GhConfig, PathBuf, RepoConfig)> {
    let gh_config = GhConfig::new()?;
    let repo_path = get_repo_path()?;
    let repo_config = read_repo_config(&repo_path)?;
    info!(?repo_config, "got repo config");
    Ok((gh_config, repo_path, repo_config))
}

fn get_repo_path() -> Result<PathBuf> {
    env::var("REPO_PATH")
        .or_else(|_| env::current_dir().map(|p| p.to_string_lossy().to_string()))
        .map(|p| Path::new(&p).to_path_buf())
        .map_err(|e| eyre::eyre!("Error getting repo path: {}", e))
}

fn get_git_info(repo_path: &Path) -> Result<(Repository, String)> {
    let git = Git::new(repo_path.to_path_buf());
    let repo = git.get_remote()?;
    let branch = git.get_branch()?;
    Ok((repo, branch))
}