use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

use clap::Parser;
use eyre::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{
    cli::Cli,
    gh_config::GhConfig,
    git::{Git, Repository},
    github::GithubClient,
    repo_config::{read_repo_config, RepoConfig},
};

pub fn setup() -> Result<(Cli, Repository, String, RepoConfig, GithubClient)> {
    let cli = Cli::parse();

    if cli.verbose {
        std::env::set_var("RUST_LOG", "info");
    }

    setup_env()?;
    let (gh_config, repo_path, repo_config) = setup_configs()?;
    let (repo, current_branch) = get_git_info(&repo_path)?;
    let site_config = gh_config.get_site_config(&repo.hostname)?;
    let github_client = GithubClient::new(site_config.oauth_token.to_string())?;
    let branch = cli.branch.as_ref().unwrap_or(&current_branch).to_string();

    Ok((cli, repo, branch, repo_config, github_client))
}

fn setup_env() -> Result<()> {
    color_eyre::install()?;

    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1");
    }

    tracing_subscriber::fmt()
        .without_time()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

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
    let git = Arc::new(Git::new(repo_path.to_path_buf()));
    let git1 = Arc::clone(&git);
    let git2 = Arc::clone(&git);
    let handle1 = thread::spawn(move || git1.get_remote());
    let handle2 = thread::spawn(move || git2.get_branch());
    let repo = handle1.join().unwrap()?;
    let branch = handle2.join().unwrap()?;
    Ok((repo, branch))
}
