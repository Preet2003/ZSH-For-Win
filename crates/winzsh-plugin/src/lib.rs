//! Plugin manifest parsing, lifecycle, trust, and dependency DAG (no network).

#![forbid(unsafe_code)]

use winzsh_core::PluginId;

/// Parsed plugin identity from `plugin.toml` (full parse lands in Phase 5 / earlier slices).
#[derive(Debug, Clone)]
pub struct PluginManifest {
    /// Plugin id.
    pub name: PluginId,
    /// Package semver (string until semver crate is wired).
    pub version: String,
}
