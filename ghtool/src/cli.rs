use clap::{Parser, Subcommand};
use eyre::Result;

use crate::git::{parse_repository_from_github, Repository};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(arg_required_else_help = true)]
#[command(color = clap::ColorChoice::Never)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Print verbose output
    #[arg(global = true)]
    #[clap(long, short)]
    pub verbose: bool,

    /// Target branch; defaults to current branch
    #[arg(global = true)]
    #[clap(long, short)]
    pub branch: Option<String>,

    /// Repository; defaults to current repository
    #[arg(global = true)]
    #[arg(value_parser = parse_repo)]
    #[clap(long, short)]
    pub repo: Option<Repository>,
}

fn parse_repo(s: &str) -> Result<Repository> {
    if s.contains('/') {
        parse_repository_from_github(s)
    } else {
        eyre::bail!("repo must be in the format owner/repo")
    }
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Get the failing tests for the current branch's pull request's checks
    Tests {
        /// Output only the file paths
        #[clap(long, short)]
        files: bool,
    },

    /// Get lint issues for the current branch's pull request's checks
    Lint {
        /// Output only the file paths
        #[clap(long, short)]
        files: bool,
    },
}
