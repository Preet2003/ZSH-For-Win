//! Plugin manifest parsing, lifecycle, trust, and materialization (no network).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use winzsh_core::{PluginId, WinzshPaths};
use winzsh_detect::DetectionReport;
use winzsh_error::{Result, message};
use winzsh_fs::{ensure_dir, read_string};

/// Parsed `plugin.toml` (V1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    /// Stable plugin id.
    pub name: String,
    /// Package version string.
    pub version: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Engine constraints.
    #[serde(default)]
    pub engines: PluginEngines,
    /// Binaries that gate materialization (any-or when empty = always).
    #[serde(default)]
    pub commands: Vec<String>,
    /// Alias map contributed by the plugin.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    /// Relative completion script paths.
    #[serde(default)]
    pub completions: Vec<String>,
    /// Relative init hook paths.
    #[serde(default)]
    pub hooks: Vec<String>,
    /// Optional theme assets (reserved).
    #[serde(default)]
    pub themes: Vec<String>,
}

/// Engine compatibility block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PluginEngines {
    /// Minimum / range note for WinZSH (informational in V1).
    #[serde(default)]
    pub winzsh: String,
}

/// An installed plugin on disk.
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    /// Install root (`~/.winzsh/plugins/<id>`).
    pub root: PathBuf,
    /// Parsed manifest.
    pub manifest: PluginManifest,
}

/// Summary row for CLI listing.
#[derive(Debug, Clone, Serialize)]
pub struct PluginListEntry {
    /// Plugin id.
    pub id: String,
    /// Package version (installed or catalog).
    pub version: String,
    /// Description.
    pub description: String,
    /// Whether present under `~/.winzsh/plugins/`.
    pub installed: bool,
    /// Whether listed in `config.plugins.enabled`.
    pub enabled: bool,
    /// First-party catalog entry.
    pub first_party: bool,
    /// Whether required commands appear present.
    pub commands_ok: bool,
}

#[derive(Clone, Copy)]
struct EmbeddedFile {
    rel: &'static str,
    contents: &'static str,
}

#[derive(Clone, Copy)]
struct EmbeddedPlugin {
    id: &'static str,
    files: &'static [EmbeddedFile],
}

const FIRST_PARTY: &[EmbeddedPlugin] = &[
    EmbeddedPlugin {
        id: "docker",
        files: &[
            EmbeddedFile {
                rel: "plugin.toml",
                contents: include_str!("../../../plugins/docker/plugin.toml"),
            },
            EmbeddedFile {
                rel: "hooks/init.ps1",
                contents: include_str!("../../../plugins/docker/hooks/init.ps1"),
            },
        ],
    },
    EmbeddedPlugin {
        id: "git",
        files: &[EmbeddedFile {
            rel: "plugin.toml",
            contents: include_str!("../../../plugins/git/plugin.toml"),
        }],
    },
    EmbeddedPlugin {
        id: "node",
        files: &[EmbeddedFile {
            rel: "plugin.toml",
            contents: include_str!("../../../plugins/node/plugin.toml"),
        }],
    },
    EmbeddedPlugin {
        id: "rust",
        files: &[EmbeddedFile {
            rel: "plugin.toml",
            contents: include_str!("../../../plugins/rust/plugin.toml"),
        }],
    },
];

/// Ids of embedded first-party plugins.
pub fn first_party_ids() -> Vec<&'static str> {
    FIRST_PARTY.iter().map(|p| p.id).collect()
}

/// Parse a `plugin.toml` document.
pub fn parse_manifest(raw: &str) -> Result<PluginManifest> {
    let manifest: PluginManifest =
        toml::from_str(raw).map_err(|e| message(format!("plugin.toml parse error: {e}")))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(m: &PluginManifest) -> Result<()> {
    let name = m.name.trim();
    if name.is_empty() {
        return Err(message("plugin name must not be empty"));
    }
    if !is_valid_plugin_id(name) {
        return Err(message(format!(
            "plugin name '{name}' is invalid (use lowercase letters, digits, -, _)"
        )));
    }
    if m.version.trim().is_empty() {
        return Err(message("plugin version must not be empty"));
    }
    for (alias, value) in &m.aliases {
        if !is_valid_alias_name(alias) {
            return Err(message(format!("plugin alias '{alias}' has an invalid name")));
        }
        if value.trim().is_empty() {
            return Err(message(format!("plugin alias '{alias}' value must not be empty")));
        }
    }
    Ok(())
}

fn is_valid_plugin_id(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn is_valid_alias_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Load an installed plugin by id.
pub fn load(paths: &WinzshPaths, id: &str) -> Result<InstalledPlugin> {
    let root = paths.plugins_dir().join(id);
    let manifest_path = root.join("plugin.toml");
    if !manifest_path.is_file() {
        return Err(message(format!(
            "plugin '{id}' is not installed (missing {})",
            manifest_path.display()
        )));
    }
    let raw = read_string(&manifest_path)?;
    let manifest = parse_manifest(&raw)?;
    if manifest.name != id {
        return Err(message(format!(
            "plugin folder '{id}' does not match manifest name '{}'",
            manifest.name
        )));
    }
    Ok(InstalledPlugin { root, manifest })
}

/// List plugins installed under `~/.winzsh/plugins/`.
pub fn list_installed(paths: &WinzshPaths) -> Result<Vec<InstalledPlugin>> {
    let dir = paths.plugins_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|source| winzsh_error::io(dir.clone(), source))?;
    for entry in entries {
        let entry = entry.map_err(|source| winzsh_error::io(dir.clone(), source))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        match load(paths, name) {
            Ok(p) => out.push(p),
            Err(e) => {
                tracing::warn!(plugin = name, error = %e, "skipping invalid installed plugin");
            }
        }
    }
    out.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(out)
}

/// Whether the plugin's command gates are satisfied (empty commands = always).
pub fn commands_ok(manifest: &PluginManifest, detected: &DetectionReport) -> bool {
    if manifest.commands.is_empty() {
        return true;
    }
    // Trust DetectionReport.commands only (populated by detect_environment).
    manifest.commands.iter().any(|cmd| {
        detected
            .commands
            .iter()
            .any(|c| c.eq_ignore_ascii_case(cmd))
    })
}

/// Enabled plugins in config order that are installed and pass command gates.
pub fn resolve_active(
    paths: &WinzshPaths,
    enabled: &[String],
    detected: &DetectionReport,
) -> Result<Vec<InstalledPlugin>> {
    let mut out = Vec::new();
    for id in enabled {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        match load(paths, id) {
            Ok(plugin) => {
                if commands_ok(&plugin.manifest, detected) {
                    out.push(plugin);
                } else {
                    tracing::info!(
                        plugin = id,
                        "skipping plugin materialization; required commands not detected"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(plugin = id, error = %e, "enabled plugin missing on disk");
            }
        }
    }
    Ok(out)
}

/// Alias pairs from active plugins (config enable order; later wins on duplicate names).
pub fn collect_aliases(plugins: &[InstalledPlugin]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for plugin in plugins {
        for (name, value) in &plugin.manifest.aliases {
            map.insert(name.clone(), value.clone());
        }
    }
    map
}

/// Install a first-party plugin by id into `~/.winzsh/plugins/<id>`.
pub fn add_first_party(paths: &WinzshPaths, id: &str) -> Result<PluginManifest> {
    let embedded = FIRST_PARTY
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| {
            message(format!(
                "unknown first-party plugin '{id}'. Available: {}",
                first_party_ids().join(", ")
            ))
        })?;
    let dest = paths.plugins_dir().join(id);
    if dest.join("plugin.toml").is_file() {
        return Err(message(format!(
            "plugin '{id}' is already installed at {}",
            dest.display()
        )));
    }
    ensure_dir(&dest)?;
    for file in embedded.files {
        let target = dest.join(file.rel);
        if let Some(parent) = target.parent() {
            ensure_dir(parent)?;
        }
        winzsh_fs::atomic_write(&target, file.contents)?;
    }
    let manifest = load(paths, id)?.manifest;
    info!(plugin = id, path = %dest.display(), "installed first-party plugin");
    let _ = write_origin(paths, id, "first-party", &manifest.version, None);
    Ok(manifest)
}

/// Install a plugin from a local directory containing `plugin.toml`.
pub fn add_from_path(paths: &WinzshPaths, src: &Path) -> Result<PluginManifest> {
    install_from_dir(paths, src, false)
}

/// Install (or replace) a plugin tree from a directory that contains `plugin.toml`.
pub fn install_from_dir(paths: &WinzshPaths, src: &Path, overwrite: bool) -> Result<PluginManifest> {
    let src = src
        .canonicalize()
        .map_err(|source| winzsh_error::io(src.to_path_buf(), source))?;
    let manifest_path = src.join("plugin.toml");
    if !manifest_path.is_file() {
        return Err(message(format!(
            "no plugin.toml in {}",
            src.display()
        )));
    }
    let raw = read_string(&manifest_path)?;
    let manifest = parse_manifest(&raw)?;
    let dest = paths.plugins_dir().join(&manifest.name);
    if dest.join("plugin.toml").is_file() {
        if !overwrite {
            return Err(message(format!(
                "plugin '{}' is already installed at {}",
                manifest.name,
                dest.display()
            )));
        }
        fs::remove_dir_all(&dest).map_err(|source| winzsh_error::io(dest.clone(), source))?;
    }
    copy_dir(&src, &dest)?;
    info!(
        plugin = %manifest.name,
        from = %src.display(),
        overwrite,
        "installed plugin from directory"
    );
    if !overwrite {
        let _ = write_origin(paths, &manifest.name, "local", &manifest.version, None);
    }
    Ok(manifest)
}

/// Write provenance metadata next to an installed plugin.
pub fn write_origin(
    paths: &WinzshPaths,
    id: &str,
    source: &str,
    version: &str,
    sha256: Option<&str>,
) -> Result<()> {
    let dest = paths.plugins_dir().join(id).join(".winzsh-origin.toml");
    let mut body = format!(
        "source = \"{source}\"\nversion = \"{version}\"\ninstalled_by = \"winzsh\"\n"
    );
    if let Some(hash) = sha256 {
        body.push_str(&format!("sha256 = \"{hash}\"\n"));
    }
    winzsh_fs::atomic_write(&dest, body)?;
    Ok(())
}

/// Read provenance source label if present (`registry`, `first-party`, `local`).
pub fn read_origin_source(paths: &WinzshPaths, id: &str) -> Option<String> {
    let path = paths.plugins_dir().join(id).join(".winzsh-origin.toml");
    let raw = read_string(&path).ok()?;
    for line in raw.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("source") {
            let rest = rest.trim().trim_start_matches('=').trim();
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

/// Remove an installed plugin directory (does not edit config).
pub fn remove(paths: &WinzshPaths, id: &str) -> Result<()> {
    let root = paths.plugins_dir().join(id);
    if !root.exists() {
        return Err(message(format!("plugin '{id}' is not installed")));
    }
    fs::remove_dir_all(&root).map_err(|source| winzsh_error::io(root.clone(), source))?;
    info!(plugin = id, "removed plugin");
    Ok(())
}

/// Build list rows for CLI (`first-party` ∪ installed).
pub fn list_entries(
    paths: &WinzshPaths,
    enabled: &[String],
    detected: &DetectionReport,
) -> Result<Vec<PluginListEntry>> {
    let installed = list_installed(paths)?;
    let installed_ids: BTreeMap<String, &InstalledPlugin> = installed
        .iter()
        .map(|p| (p.manifest.name.clone(), p))
        .collect();

    let mut seen = std::collections::BTreeSet::new();
    let mut rows = Vec::new();

    for fp in FIRST_PARTY {
        seen.insert(fp.id.to_string());
        let installed_plugin = installed_ids.get(fp.id);
        let manifest = match installed_plugin {
            Some(p) => p.manifest.clone(),
            None => parse_manifest(
                fp.files
                    .iter()
                    .find(|f| f.rel == "plugin.toml")
                    .map(|f| f.contents)
                    .unwrap_or(""),
            )?,
        };
        rows.push(PluginListEntry {
            id: fp.id.to_string(),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            installed: installed_plugin.is_some(),
            enabled: enabled.iter().any(|e| e == fp.id),
            first_party: true,
            commands_ok: commands_ok(&manifest, detected),
        });
    }

    for plugin in &installed {
        if seen.contains(&plugin.manifest.name) {
            continue;
        }
        rows.push(PluginListEntry {
            id: plugin.manifest.name.clone(),
            version: plugin.manifest.version.clone(),
            description: plugin.manifest.description.clone(),
            installed: true,
            enabled: enabled.iter().any(|e| e == &plugin.manifest.name),
            first_party: false,
            commands_ok: commands_ok(&plugin.manifest, detected),
        });
    }

    Ok(rows)
}

/// Render plugin hooks + completion scripts into the runtime module (failure-isolated).
pub fn render_powershell(plugins: &[InstalledPlugin]) -> String {
    if plugins.is_empty() {
        return "\n# --- plugins (phase 5): none enabled ---\n$script:WinZshPluginsLoaded = @()\n"
            .into();
    }

    let mut out = String::from("\n# --- plugins (phase 5) ---\n");
    out.push_str("$script:WinZshPluginsLoaded = @()\n");

    for plugin in plugins {
        let id = &plugin.manifest.name;
        out.push_str(&format!("\n# plugin: {id}\n"));
        out.push_str("try {\n");

        for rel in &plugin.manifest.completions {
            let path = plugin.root.join(rel);
            match read_string(&path) {
                Ok(body) => {
                    out.push_str(&format!("    # completions: {rel}\n"));
                    for line in body.lines() {
                        out.push_str("    ");
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                Err(e) => {
                    out.push_str(&format!(
                        "    Write-Verbose 'WinZSH plugin {id}: skip completion {rel}: {e}'\n"
                    ));
                }
            }
        }

        for rel in &plugin.manifest.hooks {
            let path = plugin.root.join(rel);
            match read_string(&path) {
                Ok(body) => {
                    out.push_str(&format!("    # hook: {rel}\n"));
                    for line in body.lines() {
                        out.push_str("    ");
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                Err(e) => {
                    out.push_str(&format!(
                        "    Write-Verbose 'WinZSH plugin {id}: skip hook {rel}: {e}'\n"
                    ));
                }
            }
        }

        out.push_str(&format!(
            "    $script:WinZshPluginsLoaded += '{id}'\n"
        ));
        out.push_str(&format!(
            "}} catch {{\n    Write-Warning \"WinZSH plugin '{id}' failed to load: $_\"\n}}\n"
        ));
    }

    out
}

/// Plugin id helper (typed).
pub fn plugin_id(name: impl Into<String>) -> PluginId {
    PluginId(name.into())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    ensure_dir(dst)?;
    let entries = fs::read_dir(src).map_err(|source| winzsh_error::io(src.to_path_buf(), source))?;
    for entry in entries {
        let entry = entry.map_err(|source| winzsh_error::io(src.to_path_buf(), source))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ft = entry
            .file_type()
            .map_err(|source| winzsh_error::io(from.clone(), source))?;
        if ft.is_dir() {
            copy_dir(&from, &to)?;
        } else if ft.is_file() {
            if let Some(parent) = to.parent() {
                ensure_dir(parent)?;
            }
            fs::copy(&from, &to).map_err(|source| winzsh_error::io(to.clone(), source))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use winzsh_fs::ensure_layout;

    #[test]
    fn parses_docker_manifest() {
        let raw = include_str!("../../../plugins/docker/plugin.toml");
        let m = parse_manifest(raw).expect("parse");
        assert_eq!(m.name, "docker");
        assert!(m.aliases.contains_key("dps"));
        assert_eq!(m.hooks, vec!["hooks/init.ps1".to_string()]);
    }

    #[test]
    fn add_first_party_roundtrip() {
        let root = std::env::temp_dir().join(format!("winzsh-plug-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = WinzshPaths::from_root(root.clone());
        ensure_layout(&paths).expect("layout");
        let m = add_first_party(&paths, "git").expect("add");
        assert_eq!(m.name, "git");
        let loaded = load(&paths, "git").expect("load");
        assert!(loaded.manifest.aliases.contains_key("gst"));
        remove(&paths, "git").expect("remove");
        assert!(load(&paths, "git").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_skips_missing_commands() {
        let root = std::env::temp_dir().join(format!("winzsh-plug2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let paths = WinzshPaths::from_root(root.clone());
        ensure_layout(&paths).expect("layout");
        add_first_party(&paths, "rust").expect("add");
        let detected = DetectionReport {
            commands: vec!["git".into()],
            ..DetectionReport::default()
        };
        let active = resolve_active(&paths, &["rust".into()], &detected).expect("resolve");
        assert!(active.is_empty());
        let with_cargo = DetectionReport {
            commands: vec!["cargo".into()],
            ..DetectionReport::default()
        };
        let active = resolve_active(&paths, &["rust".into()], &with_cargo).expect("resolve2");
        assert_eq!(active.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }
}
