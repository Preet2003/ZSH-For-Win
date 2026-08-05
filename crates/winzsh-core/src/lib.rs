//! Shared domain types, path layout, channels, and version constants.
//!
//! This crate must not depend on clap or perform network IO.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// CLI / package version embedded at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Update / release channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    /// Stable releases.
    #[default]
    Stable,
    /// Pre-release / beta channel.
    Beta,
}

/// Stable plugin identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(pub String);

/// Stable theme identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThemeId(pub String);

/// Canonical on-disk layout under the user home (typically `~/.winzsh`).
#[derive(Debug, Clone)]
pub struct WinzshPaths {
    /// Root directory (`~/.winzsh`).
    pub root: PathBuf,
}

impl WinzshPaths {
    /// Construct paths from an explicit root (tests and custom homes).
    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    /// Path to `config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// Path to runtime cache directory.
    pub fn runtime_cache(&self) -> PathBuf {
        self.root.join("cache").join("runtime")
    }

    /// Path to log directory.
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }
}
