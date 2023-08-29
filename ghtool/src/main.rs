use clap::Parser;
use cli::Commands;
use commands::{
    auth, handle_all_command, handle_command, BuildCommand, Command, LintCommand, TestCommand,
};
use eyre::Result;
use repo_config::RepoConfig;
use setup::{get_repo_config, setup};
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
    let cli = setup()?;

    match &cli.command {
        Some(Commands::Test { files }) => {
            handle_standard_command(&cli, TestCommand::from_repo_config, *files).await
        }
        Some(Commands::Lint { files }) => {
            handle_standard_command(&cli, LintCommand::from_repo_config, *files).await
        }
        Some(Commands::Build { files }) => {
            handle_standard_command(&cli, BuildCommand::from_repo_config, *files).await
        }
        Some(Commands::All {}) => handle_all_command(&cli).await,
        Some(Commands::Login { stdin }) => {
            auth::login(*stdin).await?;
            Ok(())
        }
        Some(Commands::Logout {}) => {
            auth::logout()?;
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

async fn handle_standard_command<C: Command>(
    cli: &cli::Cli,
    command_ctor: fn(&RepoConfig) -> Result<C>,
    files: bool,
) -> Result<()> {
    let (repo_config, repo, branch) = get_repo_config(cli)?;
    let command = command_ctor(&repo_config)?;
    handle_command(command, &repo, &branch, files).await
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        let _ = exit_with_error::<eyre::Error>(e);
    }
}
