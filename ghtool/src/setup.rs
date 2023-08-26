use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};

use clap::Parser;
use eyre::{Context, Result};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{
    cli::Cli,
    git::{parse_repository_from_github, Git, Repository},
    repo_config::{read_repo_config, read_repo_config_from_path, RepoConfig},
};

pub fn setup() -> Result<(Cli, Repository, String, RepoConfig)> {
    let cli = Cli::parse();

    if cli.verbose {
        std::env::set_var("RUST_LOG", "info");
    }

    setup_env()?;
    let (repo_config, repo, branch) = setup_configs(&cli)?;
    Ok((cli, repo, branch, repo_config))
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

fn setup_configs(cli: &Cli) -> Result<(RepoConfig, Repository, String)> {
    let env_repo_config = env::var("REPO_CONFIG")
        .map(|p| Path::new(&p).to_path_buf())
        .map_err(|e| eyre::eyre!("Error getting repo config path: {}", e))
        .and_then(|p| read_repo_config_from_path(&p));
    let env_repo = env::var("REPO").map(|s| parse_repository_from_github(&s).unwrap());

    // The env variables are meant to help with development. I opted to not put them as cli
    // arguments as they would make --help more noisy.
    let (repo_config, repo, branch) = match (env_repo_config, env_repo) {
        (Ok(repo_config), Ok(repo)) => {
            let branch = cli.branch.clone().ok_or_else(|| {
                eyre::eyre!("Error: --branch must be given when using REPO env variable")
            })?;
            (repo_config, repo, branch)
        }
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => {
            eyre::bail!("Error: both env variables REPO and REPO_CONFIG should be given at the same time or not at all")
        }
        (Err(_), Err(_)) => {
            let repo_path = get_repo_path()?;
            let (repo, current_branch) = get_git_info(&repo_path, cli)?;
            let repo_config = read_repo_config(&repo_path)?;
            (repo_config, repo, current_branch)
        }
    };

    info!(?repo_config, ?repo, "config");
    Ok((repo_config, repo, branch))
}

fn find_git_ancestor(mut dir: PathBuf) -> Option<PathBuf> {
    loop {
        let git_dir = dir.join(".git");
        if git_dir.is_dir() {
            return Some(dir);
        }

        if !dir.pop() {
            return None;
        }
    }
}

fn get_repo_path() -> Result<PathBuf> {
    env::var("REPO_PATH")
        .map(|p| Path::new(&p).to_path_buf())
        .or_else(|_| env::current_dir().wrap_err("Failed to get current directory"))
        .and_then(|path| find_git_ancestor(path).ok_or(eyre::eyre!("Not in git repository")))
        .map_err(|e| eyre::eyre!("Error getting repo path: {}", e))
}

fn get_git_info(repo_path: &Path, cli: &Cli) -> Result<(Repository, String)> {
    let git = Arc::new(Git::new(repo_path.to_path_buf()));
    let git1 = Arc::clone(&git);
    let handle1 = thread::spawn(move || git1.get_remote());
    let branch = match &cli.branch {
        Some(branch) => branch.clone(),
        None => {
            let git2 = Arc::clone(&git);
            let handle2 = thread::spawn(move || git2.get_branch());
            handle2.join().unwrap()?
        }
    };
    let repo = handle1.join().unwrap()?;
    Ok((repo, branch))
}
