//! Atomic IO, backup rotation, lock files, and safe path joins under `~/.winzsh`.

#![forbid(unsafe_code)]

/// Placeholder module marker for the filesystem helpers crate.
pub const CRATE_NAME: &str = "winzsh-fs";
