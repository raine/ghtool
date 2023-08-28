use clap::Parser;
use cli::Commands;
use commands::{auth, handle_all_command, handle_command, BuildCommand, LintCommand, TestCommand};
use eyre::Result;
use setup::setup;
use term::exit_with_error;

mod cache;
mod cli;
mod commands;
mod git;
mod github;
mod repo_config;
mod setup;
mod spinner;
mod term;
mod token_store;

async fn run() -> Result<()> {
    let (cli, repo, branch, repo_config) = setup()?;

    match &cli.command {
        Some(Commands::Test { files }) => {
            let command = TestCommand::from_repo_config(&repo_config)?;
            handle_command(command, &repo, &branch, *files).await
        }
        Some(Commands::Lint { files }) => {
            let command = LintCommand::from_repo_config(&repo_config)?;
            handle_command(command, &repo, &branch, *files).await
        }
        Some(Commands::Build { files }) => {
            let command = BuildCommand::from_repo_config(&repo_config)?;
            handle_command(command, &repo, &branch, *files).await
        }
        Some(Commands::All {}) => handle_all_command(&repo_config, &repo, &branch).await,
        Some(Commands::Login { stdin }) => {
            auth::login(&repo.hostname, *stdin).await?;
            Ok(())
        }
        Some(Commands::Logout {}) => {
            auth::logout(&repo.hostname)?;
            Ok(())
        }
        None => {
            // Show help if no command is given. arg_required_else_help clap thing is supposed to
            // do this but that doesn't work if some arguments, but no command, are given
            cli::Cli::parse_from(["--help"]);
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        let _ = exit_with_error::<eyre::Error>(e);
    }
}
