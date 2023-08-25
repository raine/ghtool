use clap::{Parser, Subcommand};

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
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Get the failing tests for the current branch's pull request's checks
    Test {
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

    /// Get typechecking issues for the current branch's pull request's checks
    Typecheck {
        /// Output only the file paths
        #[clap(long, short)]
        files: bool,
    },

    /// Authenticate ghtool with GitHub API
    Login {},

    /// Deauthenticate ghtool with GitHub API
    Logout {},
}
