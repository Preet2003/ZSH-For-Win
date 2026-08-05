//! Schema-versioned `config.toml` load, validation, and migrations.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use winzsh_core::Channel;

/// Current supported config schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Root user configuration document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Schema version for migrations.
    pub schema_version: u32,
    /// Active theme id.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Feature toggles.
    #[serde(default)]
    pub features: Features,
    /// Update preferences.
    #[serde(default)]
    pub update: UpdateConfig,
    /// Telemetry preferences (default off).
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

fn default_theme() -> String {
    "modern".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            theme: default_theme(),
            features: Features::default(),
            update: UpdateConfig::default(),
            telemetry: TelemetryConfig::default(),
        }
    }
}

/// Feature flags stored in config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    /// Enable autosuggestions.
    #[serde(default = "default_true")]
    pub autosuggestions: bool,
    /// Enable syntax highlighting.
    #[serde(default = "default_true")]
    pub syntax: bool,
    /// Enable enhanced history.
    #[serde(default = "default_true")]
    pub history: bool,
    /// Enable AI features (Phase 6).
    #[serde(default)]
    pub ai: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            autosuggestions: true,
            syntax: true,
            history: true,
            ai: false,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Update-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Release channel.
    #[serde(default)]
    pub channel: Channel,
    /// Whether to check for updates on start.
    #[serde(default = "default_true")]
    pub check_on_start: bool,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            channel: Channel::Stable,
            check_on_start: true,
        }
    }
}

/// Telemetry configuration (always default-off).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelemetryConfig {
    /// Explicit opt-in only.
    #[serde(default)]
    pub enabled: bool,
}
