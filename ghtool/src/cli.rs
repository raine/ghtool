use clap::{Parser, Subcommand};

#[derive(Parser)]
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
}

#[derive(Subcommand)]
pub enum Commands {
    /// Get the failing tests for the current branch's pull request's checks
    FailingTests {
        /// Output only the file paths
        #[clap(long, short)]
        files: bool,
    },

    /// Get lint issues for the current branch's pull request's checks
    Lint {},
}
