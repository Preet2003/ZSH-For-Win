//! Domain errors and CLI-boundary reporting helpers for WinZSH.

#![forbid(unsafe_code)]

use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

/// Top-level error type used across WinZSH libraries and the CLI boundary.
#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    /// Generic IO failure with path context.
    #[error("IO error at {path}: {source}")]
    #[diagnostic(code(winzsh::io))]
    Io {
        /// Path involved in the failure.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },

    /// Configuration parse or validation failure.
    #[error("config error: {message}")]
    #[diagnostic(code(winzsh::config))]
    Config {
        /// Human-readable details.
        message: String,
    },

    /// WinZSH is not installed (or install state is missing).
    #[error("WinZSH is not installed; run `winzsh install`")]
    #[diagnostic(code(winzsh::not_installed))]
    NotInstalled,

    /// Environment / tool detection failure.
    #[error("detection error: {message}")]
    #[diagnostic(code(winzsh::detect))]
    Detect {
        /// Human-readable details.
        message: String,
    },

    /// PowerShell profile integration failure.
    #[error("profile error: {message}")]
    #[diagnostic(code(winzsh::profile))]
    Profile {
        /// Human-readable details.
        message: String,
    },

    /// Runtime generation failure.
    #[error("runtime generation error: {message}")]
    #[diagnostic(code(winzsh::runtime))]
    Runtime {
        /// Human-readable details.
        message: String,
    },

    /// Installer state conflict (e.g. already installed without --force).
    #[error("{message}")]
    #[diagnostic(code(winzsh::state))]
    State {
        /// Human-readable details.
        message: String,
    },

    /// Catch-all message for unusual failures.
    #[error("{0}")]
    #[diagnostic(code(winzsh::general))]
    Message(String),
}

/// Result alias for WinZSH libraries.
pub type Result<T> = std::result::Result<T, Error>;

/// Build a general message error.
pub fn message(msg: impl Into<String>) -> Error {
    Error::Message(msg.into())
}

/// Build a config error.
pub fn config(msg: impl Into<String>) -> Error {
    Error::Config {
        message: msg.into(),
    }
}

/// Build a detect error.
pub fn detect(msg: impl Into<String>) -> Error {
    Error::Detect {
        message: msg.into(),
    }
}

/// Build a profile error.
pub fn profile(msg: impl Into<String>) -> Error {
    Error::Profile {
        message: msg.into(),
    }
}

/// Build a runtime error.
pub fn runtime(msg: impl Into<String>) -> Error {
    Error::Runtime {
        message: msg.into(),
    }
}

/// Build a state conflict error.
pub fn state(msg: impl Into<String>) -> Error {
    Error::State {
        message: msg.into(),
    }
}

/// Map an IO error with path context.
pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}

impl Error {
    /// Suggested process exit code for this error.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Config { .. } | Self::Message(_) | Self::Io { .. } | Self::Runtime { .. } => 1,
            Self::NotInstalled | Self::State { .. } | Self::Profile { .. } => 3,
            Self::Detect { .. } => 1,
        }
    }
}
