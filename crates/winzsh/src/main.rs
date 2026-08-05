//! WinZSH binary entrypoint.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    winzsh_log::init_default();
    winzsh_cli::run()
}
