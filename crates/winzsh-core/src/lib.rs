//! Shared domain types, path layout, channels, and version constants.
//!
//! This crate must not depend on clap or perform network IO.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;
use uuid::Uuid;
use winzsh_error::{Result, io};

/// CLI / package version embedded at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Environment variable overriding the WinZSH home directory.
pub const HOME_ENV: &str = "WINZSH_HOME";

/// Environment variable overriding the PowerShell profile path (tests / advanced use).
pub const PROFILE_ENV: &str = "WINZSH_PROFILE_PATH";

/// When set to `1`, the profile hook loads the WinZSH runtime (nested `zsh-for-win` session).
/// Plain PowerShell stays stock unless this is set.
pub const SHELL_ENV: &str = "WINZSH_SHELL";

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

    /// Discover paths from `WINZSH_HOME` or the user home directory.
    pub fn discover() -> Result<Self> {
        if let Ok(override_home) = std::env::var(HOME_ENV) {
            if !override_home.trim().is_empty() {
                return Ok(Self::from_root(PathBuf::from(override_home)));
            }
        }
        let home = dirs::home_dir()
            .ok_or_else(|| winzsh_error::message("could not determine user home directory"))?;
        Ok(Self::from_root(home.join(".winzsh")))
    }

    /// Path to `config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    /// Path to config backup.
    pub fn config_backup(&self) -> PathBuf {
        self.root.join("config.toml.bak")
    }

    /// Path to `state.json`.
    pub fn state_file(&self) -> PathBuf {
        self.root.join("state.json")
    }

    /// Path to runtime cache directory.
    pub fn runtime_cache(&self) -> PathBuf {
        self.root.join("cache").join("runtime")
    }

    /// Path to generated module `.psm1`.
    pub fn runtime_module(&self) -> PathBuf {
        self.runtime_cache().join("WinZSH.psm1")
    }

    /// Path to generated module manifest `.psd1`.
    pub fn runtime_manifest(&self) -> PathBuf {
        self.runtime_cache().join("WinZSH.psd1")
    }

    /// Path to runtime lockfile.
    pub fn runtime_lock(&self) -> PathBuf {
        self.runtime_cache().join("runtime.lock.json")
    }

    /// Path to log directory.
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Path to primary log file.
    pub fn log_file(&self) -> PathBuf {
        self.logs_dir().join("winzsh.log")
    }

    /// Path to profile backups directory.
    pub fn profile_backups(&self) -> PathBuf {
        self.root.join("backups").join("profile")
    }

    /// Path to plugins directory.
    pub fn plugins_dir(&self) -> PathBuf {
        self.root.join("plugins")
    }

    /// Path to themes directory.
    pub fn themes_dir(&self) -> PathBuf {
        self.root.join("themes")
    }

    /// Path to locks directory.
    pub fn locks_dir(&self) -> PathBuf {
        self.root.join("locks")
    }

    /// Global WinZSH shell-active lock (`locks/shell.active`).
    ///
    /// Created by `zsh-for-win`, removed when any nested session `exit`s so every
    /// other PowerShell terminal drops back to stock.
    pub fn shell_active_lock(&self) -> PathBuf {
        self.locks_dir().join("shell.active")
    }

    /// Path to history directory.
    pub fn history_dir(&self) -> PathBuf {
        self.root.join("history")
    }

    /// Append-only history spool written by the PowerShell runtime.
    pub fn history_spool(&self) -> PathBuf {
        self.history_dir().join("spool.jsonl")
    }

    /// Compacted history store (JSONL).
    pub fn history_store(&self) -> PathBuf {
        self.history_dir().join("history.jsonl")
    }

    /// Whether a prior install appears present (`state.json` exists).
    pub fn is_installed(&self) -> bool {
        self.state_file().is_file()
    }
}

/// Persistent install / runtime state written to `state.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// Unique install identifier.
    pub install_id: String,
    /// Installed CLI version string.
    pub installed_version: String,
    /// Config schema version at last successful install/update of state.
    pub config_schema_version: u32,
    /// RFC3339 timestamp of initial install.
    pub installed_at: String,
    /// RFC3339 timestamp of last modification.
    pub updated_at: String,
}

impl State {
    /// Create a fresh install state for the current version.
    pub fn new_install(config_schema_version: u32) -> Self {
        let now = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "unknown".to_string());
        Self {
            install_id: Uuid::new_v4().to_string(),
            installed_version: VERSION.to_string(),
            config_schema_version,
            installed_at: now.clone(),
            updated_at: now,
        }
    }

    /// Load state from disk.
    pub fn load(paths: &WinzshPaths) -> Result<Self> {
        let path = paths.state_file();
        let raw = std::fs::read_to_string(&path).map_err(|source| io(path.clone(), source))?;
        serde_json::from_str(&raw).map_err(|e| {
            winzsh_error::message(format!("invalid state.json at {}: {e}", path.display()))
        })
    }

    /// Write state to disk (caller should use atomic write via `winzsh-fs` when available).
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| winzsh_error::message(format!("serialize state.json: {e}")))
    }
}
