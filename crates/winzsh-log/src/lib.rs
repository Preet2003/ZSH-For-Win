//! Tracing subscriber setup and redaction filters for WinZSH.

#![forbid(unsafe_code)]

use tracing_subscriber::EnvFilter;

/// Initialize a default tracing subscriber for the CLI process.
///
/// Safe to call once from `main`. Subsequent calls are ignored if a global
/// subscriber is already set.
pub fn init_default() {
    let filter = EnvFilter::try_from_env("WINZSH_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
