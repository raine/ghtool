use clap::Parser;
use cli::Commands;
use commands::{handle_command, LintCommand, TestCommand, TypecheckCommand};
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
        Some(Commands::Tests { files }) => {
            let command = TestCommand::from_repo_config(repo_config)?;
            handle_command(command, &github_client, &repo, &branch, *files).await
        }
        Some(Commands::Lint { files }) => {
            let command = LintCommand::from_repo_config(repo_config)?;
            handle_command(command, &github_client, &repo, &branch, *files).await
        }
        Some(Commands::Typecheck { files }) => {
            let command = TypecheckCommand::from_repo_config(repo_config)?;
            handle_command(command, &github_client, &repo, &branch, *files).await
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
