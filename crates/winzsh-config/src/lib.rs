//! Schema-versioned `config.toml` load, validation, and migrations.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use winzsh_core::{Channel, WinzshPaths};
use winzsh_error::{Result, config};
use winzsh_fs::{atomic_write, read_string};

/// Current supported config schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Root user configuration document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Schema version for migrations.
    pub schema_version: u32,
    /// Active theme id.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Feature toggles.
    #[serde(default)]
    pub features: Features,
    /// Prompt preferences.
    #[serde(default)]
    pub prompt: PromptConfig,
    /// User aliases (highest precedence).
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    /// History preferences.
    #[serde(default)]
    pub history: HistoryConfig,
    /// Smart-shell integrations (fzf / zoxide).
    #[serde(default)]
    pub smart: SmartConfig,
    /// Completion packs (Phase 4).
    #[serde(default)]
    pub completions: CompletionsConfig,
    /// Enabled plugins (Phase 5 fills behavior; stored from Phase 1).
    #[serde(default)]
    pub plugins: PluginsConfig,
    /// AI helper preferences (Phase 6).
    #[serde(default)]
    pub ai: AiConfig,
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
            prompt: PromptConfig::default(),
            aliases: BTreeMap::new(),
            history: HistoryConfig::default(),
            smart: SmartConfig::default(),
            completions: CompletionsConfig::default(),
            plugins: PluginsConfig::default(),
            ai: AiConfig::default(),
            update: UpdateConfig::default(),
            telemetry: TelemetryConfig::default(),
        }
    }
}

/// Feature flags stored in config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Features {
    /// Enable autosuggestions (Phase 3).
    #[serde(default = "default_true")]
    pub autosuggestions: bool,
    /// Enable syntax highlighting (Phase 3).
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

/// Prompt configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptConfig {
    /// Show git branch / dirty marker.
    #[serde(default = "default_true")]
    pub git: bool,
    /// Soft latency budget in milliseconds.
    #[serde(default = "default_budget")]
    pub budget_ms: u64,
}

fn default_budget() -> u64 {
    20
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            git: true,
            budget_ms: default_budget(),
        }
    }
}

/// History configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryConfig {
    /// Master enable (also gated by `features.history`).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Max compacted entries retained.
    #[serde(default = "default_history_max")]
    pub max_entries: usize,
}

fn default_history_max() -> usize {
    10_000
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: default_history_max(),
        }
    }
}

/// Fuzzy / jump-tool integrations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SmartConfig {
    /// Enable Ctrl+R fzf history when `fzf` is installed.
    #[serde(default = "default_true")]
    pub fzf: bool,
    /// Initialize zoxide when installed.
    #[serde(default = "default_true")]
    pub zoxide: bool,
}

impl Default for SmartConfig {
    fn default() -> Self {
        Self {
            fzf: true,
            zoxide: true,
        }
    }
}

/// Completion pack configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletionsConfig {
    /// Master enable for Tab completions.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// If non-empty, only these pack ids are enabled (e.g. `docker`, `kubectl`).
    #[serde(default)]
    pub only: Vec<String>,
}

impl Default for CompletionsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            only: Vec::new(),
        }
    }
}

/// Plugin enablement list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PluginsConfig {
    /// Enabled plugin ids.
    #[serde(default)]
    pub enabled: Vec<String>,
}

/// AI provider configuration (master switch is `features.ai`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiConfig {
    /// `local` (offline) or `openai` (OpenAI-compatible HTTP).
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    /// Model id for cloud provider.
    #[serde(default = "default_ai_model")]
    pub model: String,
    /// API base URL for OpenAI-compatible endpoints.
    #[serde(default = "default_ai_base")]
    pub api_base: String,
}

fn default_ai_provider() -> String {
    "local".into()
}

fn default_ai_model() -> String {
    "gpt-4o-mini".into()
}

fn default_ai_base() -> String {
    "https://api.openai.com/v1".into()
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_ai_provider(),
            model: default_ai_model(),
            api_base: default_ai_base(),
        }
    }
}

/// Update-related configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TelemetryConfig {
    /// Explicit opt-in only.
    #[serde(default)]
    pub enabled: bool,
}

/// Validate a config document.
pub fn validate(cfg: &Config) -> Result<()> {
    if cfg.schema_version == 0 {
        return Err(config("schema_version must be >= 1"));
    }
    if cfg.schema_version > SCHEMA_VERSION {
        return Err(config(format!(
            "schema_version {} is newer than this CLI supports ({SCHEMA_VERSION})",
            cfg.schema_version
        )));
    }
    if cfg.theme.trim().is_empty() {
        return Err(config("theme must not be empty"));
    }
    if cfg.telemetry.enabled {
        return Err(config(
            "telemetry.enabled=true is not supported yet; keep telemetry disabled",
        ));
    }
    if cfg.prompt.budget_ms == 0 {
        return Err(config("prompt.budget_ms must be >= 1"));
    }
    let provider = cfg.ai.provider.trim().to_ascii_lowercase();
    if provider != "local" && provider != "openai" {
        return Err(config(
            "ai.provider must be \"local\" or \"openai\"",
        ));
    }
    if cfg.ai.model.trim().is_empty() {
        return Err(config("ai.model must not be empty"));
    }
    if cfg.ai.api_base.trim().is_empty() {
        return Err(config("ai.api_base must not be empty"));
    }
    Ok(())
}

/// Apply in-place migrations toward [`SCHEMA_VERSION`].
pub fn migrate(mut cfg: Config) -> Result<Config> {
    if cfg.schema_version == 0 {
        cfg.schema_version = 1;
    }
    while cfg.schema_version < SCHEMA_VERSION {
        cfg.schema_version += 1;
    }
    validate(&cfg)?;
    Ok(cfg)
}

/// Parse config TOML from a string.
pub fn parse(raw: &str) -> Result<Config> {
    let cfg: Config = toml::from_str(raw).map_err(|e| config(format!("parse error: {e}")))?;
    migrate(cfg)
}

/// Load config from disk.
pub fn load(paths: &WinzshPaths) -> Result<Config> {
    let path = paths.config_file();
    if !path.is_file() {
        return Err(config(format!(
            "missing config at {}; run `winzsh install`",
            path.display()
        )));
    }
    let raw = read_string(&path)?;
    parse(&raw)
}

/// Serialize and atomically write config.
pub fn save(paths: &WinzshPaths, cfg: &Config) -> Result<()> {
    validate(cfg)?;
    let rendered = toml::to_string_pretty(cfg).map_err(|e| config(format!("serialize: {e}")))?;
    atomic_write(&paths.config_file(), rendered)?;
    Ok(())
}

/// Load config or write defaults if missing.
pub fn load_or_init(paths: &WinzshPaths) -> Result<Config> {
    if paths.config_file().is_file() {
        load(paths)
    } else {
        let cfg = Config::default();
        save(paths, &cfg)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_roundtrip() {
        let cfg = Config::default();
        let raw = toml::to_string_pretty(&cfg).expect("ser");
        let parsed = parse(&raw).expect("parse");
        assert_eq!(parsed.theme, "modern");
        assert!(parsed.prompt.git);
        assert!(!parsed.telemetry.enabled);
    }

    #[test]
    fn rejects_empty_theme() {
        let mut cfg = Config::default();
        cfg.theme.clear();
        assert!(validate(&cfg).is_err());
    }
}
