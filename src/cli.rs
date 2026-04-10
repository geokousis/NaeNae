use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "naenae",
    about = "Monitor a command and send lifecycle or regex-triggered notifications to Discord."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Run {
        #[arg(short, long, default_value = "naenae.toml")]
        config: PathBuf,
        #[arg(long)]
        no_pty: bool,
        #[arg(long)]
        quiet: bool,
        #[arg(long)]
        command: Option<String>,
        #[arg(long)]
        cwd: Option<PathBuf>,
    },
    Ps,
    Attach {
        #[arg(short, long, default_value = "naenae.toml")]
        config: PathBuf,
        #[arg(long)]
        pid: i32,
        #[arg(long)]
        log_file: Option<PathBuf>,
    },
}
