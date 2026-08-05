//! Domain errors and CLI-boundary reporting helpers for WinZSH.
//!
//! Library crates define typed errors here or re-export crate-local `thiserror` enums
//! that convert into user-facing reports at the CLI boundary only.

#![forbid(unsafe_code)]

use miette::Diagnostic;
use thiserror::Error;

/// Top-level error type used at process boundaries until per-crate errors land.
#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    /// Placeholder for scaffolding; replace with typed domain errors in Phase 1+.
    #[error("{0}")]
    #[diagnostic(code(winzsh::scaffold))]
    Message(String),
}

/// Result alias for WinZSH libraries.
pub type Result<T> = std::result::Result<T, Error>;

/// Build a scaffold/message error.
pub fn message(msg: impl Into<String>) -> Error {
    Error::Message(msg.into())
}
