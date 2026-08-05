//! Tracing subscriber setup for WinZSH (stderr + optional file under `~/.winzsh/logs`).

#![forbid(unsafe_code)]

use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;
use winzsh_core::WinzshPaths;
use winzsh_error::Result;
use winzsh_fs::ensure_dir;

static FILE_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Logging initialization options.
#[derive(Debug, Clone, Default)]
pub struct LogOptions {
    /// Increase verbosity (`debug` when true).
    pub verbose: bool,
    /// Also write to `~/.winzsh/logs/winzsh.log` when paths are provided.
    pub paths: Option<WinzshPaths>,
}

/// Initialize tracing. Safe to call once; later calls are ignored.
pub fn init(options: LogOptions) -> Result<()> {
    let default_level = if options.verbose { "debug" } else { "info" };
    let filter =
        EnvFilter::try_from_env("WINZSH_LOG").unwrap_or_else(|_| EnvFilter::new(default_level));

    let stderr_layer = fmt::layer().with_writer(std::io::stderr).with_target(false);

    if let Some(paths) = options.paths {
        ensure_dir(&paths.logs_dir())?;
        let file_appender = tracing_appender::rolling::never(paths.logs_dir(), "winzsh.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let _ = FILE_GUARD.set(guard);
        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_target(true);
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .with(file_layer)
            .try_init();
    } else {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .try_init();
    }
    Ok(())
}

/// Initialize a default stderr-only subscriber (used by binary before paths are known).
pub fn init_default() {
    let _ = init(LogOptions::default());
}
