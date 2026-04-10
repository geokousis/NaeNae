mod cli;
mod config;
mod formatting;
mod notifier;
mod procfs;
mod rules;
mod run;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::config::load_config;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            config,
            no_pty,
            quiet,
            command,
            cwd,
        } => run::run_command_mode(load_config(&config).await?, no_pty, quiet, command, cwd).await,
        Commands::Ps => procfs::list_processes().await,
        Commands::Attach {
            config,
            pid,
            log_file,
        } => run::attach_mode(load_config(&config).await?, pid, log_file).await,
    }
}
