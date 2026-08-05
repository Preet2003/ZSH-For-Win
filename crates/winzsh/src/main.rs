//! WinZSH binary entrypoint.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    // Early stderr logger; CLI re-inits with file sink after path discovery.
    winzsh_log::init_default();
    winzsh_cli::run()
}
