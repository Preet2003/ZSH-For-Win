//! CLI command tree, output modes, and exit-code mapping.
//!
//! Business logic lives in sibling crates; this crate stays thin.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use std::process::ExitCode;
use winzsh_core::VERSION;

/// WinZSH — Oh My Zsh–style developer experience for Windows shells.
#[derive(Debug, Parser)]
#[command(name = "winzsh", version = VERSION, about, long_about = None)]
struct Cli {
    /// Emit machine-readable JSON on stdout where supported.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Show scaffold status (Phase 1 features land next).
    Status,
}

/// Parse CLI args and dispatch. Returns a process exit code.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Status) {
        Commands::Status => {
            if cli.json {
                println!(
                    "{{\"name\":\"winzsh\",\"version\":\"{VERSION}\",\"phase\":\"architecture-scaffold\"}}"
                );
            } else {
                println!("WinZSH {VERSION}");
                println!("Architecture scaffold ready. Phase 1 feature work has not started.");
            }
            ExitCode::SUCCESS
        }
    }
}
