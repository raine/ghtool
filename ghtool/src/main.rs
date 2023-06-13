use cli::Commands;
use eyre::Result;
use setup::setup;
use term::exit_with_error;

mod cli;
mod commands;
mod gh_config;
mod git;
mod github;
mod repo_config;
mod setup;
mod term;

async fn run() -> Result<()> {
    let (cli, repo, branch, repo_config) = setup()?;

    let _ = match &cli.command {
        Some(Commands::FailingTests { files }) => {
            commands::failing_tests(&repo, &branch, &repo_config, *files).await
        }
        None => Ok(()),
    };

    std::process::exit(0);
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        let _ = exit_with_error::<eyre::Error>(e);
    }
}