use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(arg_required_else_help = true)]
#[command(color = clap::ColorChoice::Never)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Get the failing tests for the current branch's pull request's checks
    FailingTests {
        /// Show a summary instead of failing test files
        #[clap(long, short)]
        summary: bool,
    },
}
