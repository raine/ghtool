use cli::Commands;
use eyre::Result;
use setup::setup;
use term::exit_with_error;

mod cache;
mod cli;
mod commands;
mod gh_config;
mod git;
mod github;
mod repo_config;
mod setup;
mod spinner;
mod term;

async fn run() -> Result<()> {
    let (cli, repo, branch, repo_config, github_client) = setup()?;

    match &cli.command {
        Some(Commands::FailingTests { files }) => {
            commands::failing_tests(&github_client, &repo, &branch, &repo_config, *files).await
        }
        Some(Commands::Lint {}) => {
            commands::lint(&github_client, &repo, &branch, &repo_config).await
        }
        None => Ok(()),
    }
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        let _ = exit_with_error::<eyre::Error>(e);
    }
}
