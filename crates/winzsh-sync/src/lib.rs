//! Settings sync: export/import bundles, push/pull to a shared path or HTTPS pull.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use tracing::info;
use winzsh_config::{self as config, Config, SyncConfig};
use winzsh_core::{State, VERSION, WinzshPaths};
use winzsh_error::{Result, message};
use winzsh_fs::{atomic_write, ensure_dir, read_string};
use winzsh_history::{self as history, HistoryEntry, HistoryQuery};
use winzsh_plugin::{self as plugin};

/// Sync bundle schema version.
pub const BUNDLE_SCHEMA: u32 = 1;

/// Options for building an export bundle.
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// Include installed plugin file trees.
    pub include_plugins: bool,
    /// Include compacted history entries.
    pub include_history: bool,
    /// Max history entries (most recent) when including history.
    pub history_limit: usize,
}

impl ExportOptions {
    /// Merge CLI flags with `[sync]` config defaults.
    pub fn from_config(cfg: &SyncConfig) -> Self {
        Self {
            include_plugins: cfg.include_plugins,
            include_history: cfg.include_history,
            history_limit: 5_000,
        }
    }
}

/// Options for applying an import.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Overwrite existing config without requiring `--force` semantics in callers.
    pub force: bool,
    /// Prefer bundled plugin files over re-fetching from registry.
    pub prefer_bundled_plugins: bool,
}

/// Portable sync document (JSON).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncBundle {
    /// Bundle schema.
    pub schema_version: u32,
    /// CLI version that produced the bundle.
    pub winzsh_version: String,
    /// RFC3339 creation time.
    pub created_at: String,
    /// Source install id (informational).
    #[serde(default)]
    pub install_id: String,
    /// Sanitized `config.toml` text.
    pub config_toml: String,
    /// Optional plugin id → relative path → file contents.
    #[serde(default)]
    pub plugins: BTreeMap<String, BTreeMap<String, String>>,
    /// Optional history entries (oldest→newest).
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
}

/// Outcome of export/push.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportReport {
    /// Destination path or URL label.
    pub destination: String,
    /// SHA-256 of written JSON.
    pub sha256: String,
    /// Whether plugins were included.
    pub included_plugins: bool,
    /// History entry count included.
    pub history_count: usize,
    /// Steps.
    pub steps: Vec<String>,
}

/// Outcome of import/pull.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportReport {
    /// Source path or URL.
    pub source: String,
    /// Theme after import.
    pub theme: String,
    /// Enabled plugins after import.
    pub enabled_plugins: Vec<String>,
    /// Plugin ids restored from bundle files.
    pub plugins_restored: Vec<String>,
    /// History entries appended.
    pub history_imported: usize,
    /// Steps.
    pub steps: Vec<String>,
}

/// Last sync metadata under `~/.winzsh/sync-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SyncState {
    /// Last successful export/push time.
    #[serde(default)]
    pub last_export_at: String,
    /// Last successful import/pull time.
    #[serde(default)]
    pub last_import_at: String,
    /// Last destination used.
    #[serde(default)]
    pub last_destination: String,
    /// Bundle hash from last export.
    #[serde(default)]
    pub last_sha256: String,
}

/// Status view for CLI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncStatus {
    /// Configured destination.
    pub destination: String,
    /// Default include_plugins.
    pub include_plugins: bool,
    /// Default include_history.
    pub include_history: bool,
    /// Persisted sync state.
    pub state: SyncState,
}

/// Read sync status from config + state file.
pub fn status(paths: &WinzshPaths, cfg: &SyncConfig) -> SyncStatus {
    SyncStatus {
        destination: cfg.destination.clone(),
        include_plugins: cfg.include_plugins,
        include_history: cfg.include_history,
        state: load_state(paths),
    }
}

/// Build a sync bundle from the current install.
pub fn build_bundle(paths: &WinzshPaths, opts: &ExportOptions) -> Result<SyncBundle> {
    let cfg = config::load(paths)?;
    let sanitized = sanitize_config(cfg);
    let config_toml = toml::to_string_pretty(&sanitized)
        .map_err(|e| message(format!("serialize config for sync: {e}")))?;

    let install_id = State::load(paths)
        .map(|s| s.install_id)
        .unwrap_or_default();

    let mut plugins = BTreeMap::new();
    if opts.include_plugins {
        for installed in plugin::list_installed(paths)? {
            let id = installed.manifest.name.clone();
            let files = collect_plugin_files(&installed.root)?;
            if !files.is_empty() {
                plugins.insert(id, files);
            }
        }
    }

    let mut history_entries = Vec::new();
    if opts.include_history {
        let limit = if opts.history_limit == 0 {
            5_000
        } else {
            opts.history_limit
        };
        // query returns newest-first; reverse for stable chronological export.
        let mut entries = history::query(
            paths,
            &HistoryQuery {
                limit,
                contains: None,
            },
        )?;
        entries.reverse();
        history_entries = entries;
    }

    Ok(SyncBundle {
        schema_version: BUNDLE_SCHEMA,
        winzsh_version: VERSION.to_string(),
        created_at: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "unknown".into()),
        install_id,
        config_toml,
        plugins,
        history: history_entries,
    })
}

/// Export bundle to a local JSON path (file or directory → `winzsh-sync.json`).
pub fn export_to_path(
    paths: &WinzshPaths,
    dest: &Path,
    opts: &ExportOptions,
) -> Result<ExportReport> {
    let bundle = build_bundle(paths, opts)?;
    let path = resolve_bundle_file(dest, true)?;
    let json = serde_json::to_string_pretty(&bundle)
        .map_err(|e| message(format!("serialize sync bundle: {e}")))?;
    let sha = sha256_hex(json.as_bytes());
    atomic_write(&path, &json)?;

    let mut state = load_state(paths);
    state.last_export_at = bundle.created_at.clone();
    state.last_destination = path.display().to_string();
    state.last_sha256 = sha.clone();
    save_state(paths, &state)?;

    info!(path = %path.display(), sha = %sha, "exported sync bundle");
    Ok(ExportReport {
        destination: path.display().to_string(),
        sha256: sha,
        included_plugins: opts.include_plugins,
        history_count: bundle.history.len(),
        steps: vec![format!("wrote {}", path.display())],
    })
}

/// Import a bundle from a local path or HTTPS URL.
pub fn import_from(
    paths: &WinzshPaths,
    source: &str,
    opts: &ImportOptions,
) -> Result<ImportReport> {
    let (label, raw) = read_bundle_bytes(source)?;
    let bundle: SyncBundle = serde_json::from_slice(&raw)
        .map_err(|e| message(format!("parse sync bundle: {e}")))?;
    if bundle.schema_version != BUNDLE_SCHEMA {
        return Err(message(format!(
            "unsupported sync schema_version {}",
            bundle.schema_version
        )));
    }

    let mut steps = Vec::new();
    if paths.config_file().is_file() {
        let backup_dir = paths.root.join("backups").join("sync");
        ensure_dir(&backup_dir)?;
        let stamp = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "backup".into())
            .replace(':', "");
        let backup = backup_dir.join(format!("config-{stamp}.toml"));
        fs::copy(paths.config_file(), &backup)
            .map_err(|source| winzsh_error::io(backup.clone(), source))?;
        steps.push(format!("backed up config to {}", backup.display()));
    } else if !opts.force {
        // Allow first-time import without prior config when force or missing.
    }

    let mut cfg = config::parse(&bundle.config_toml)?;
    // Keep machine-local paths empty after import.
    cfg = sanitize_config(cfg);
    config::save(paths, &cfg)?;
    steps.push("wrote config.toml".into());

    let mut plugins_restored = Vec::new();
    if opts.prefer_bundled_plugins || !bundle.plugins.is_empty() {
        for (id, files) in &bundle.plugins {
            let dest = paths.plugins_dir().join(id);
            if dest.exists() {
                fs::remove_dir_all(&dest)
                    .map_err(|source| winzsh_error::io(dest.clone(), source))?;
            }
            ensure_dir(&dest)?;
            for (rel, contents) in files {
                let target = dest.join(rel);
                if let Some(parent) = target.parent() {
                    ensure_dir(parent)?;
                }
                atomic_write(&target, contents)?;
            }
            let _ = plugin::write_origin(paths, id, "sync", "bundle", None);
            plugins_restored.push(id.clone());
            steps.push(format!("restored plugin '{id}' from bundle"));
        }
    }

    // Ensure enabled plugins exist (first-party / already restored).
    for id in &cfg.plugins.enabled {
        if plugin::load(paths, id).is_ok() {
            continue;
        }
        if plugin::first_party_ids().contains(&id.as_str()) {
            match plugin::add_first_party(paths, id) {
                Ok(_) => steps.push(format!("installed first-party plugin '{id}'")),
                Err(e) => steps.push(format!("skip first-party '{id}': {e}")),
            }
        } else {
            steps.push(format!(
                "plugin '{id}' enabled but missing — run `winzsh plugin add {id}`"
            ));
        }
    }

    let mut history_imported = 0;
    if !bundle.history.is_empty() {
        history::ensure(paths)?;
        for entry in &bundle.history {
            history::append(paths, entry)?;
            history_imported += 1;
        }
        steps.push(format!("imported {history_imported} history entries"));
    }

    let mut state = load_state(paths);
    state.last_import_at = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    state.last_destination = label.clone();
    state.last_sha256 = sha256_hex(&raw);
    save_state(paths, &state)?;

    info!(source = %label, "imported sync bundle");
    Ok(ImportReport {
        source: label,
        theme: cfg.theme,
        enabled_plugins: cfg.plugins.enabled,
        plugins_restored,
        history_imported,
        steps,
    })
}

/// Push to configured destination (local path only).
pub fn push(paths: &WinzshPaths, cfg: &SyncConfig, opts: &ExportOptions) -> Result<ExportReport> {
    let dest = resolve_destination(cfg)?;
    if dest.starts_with("https://") || dest.starts_with("http://") {
        return Err(message(
            "push to HTTPS is not supported — set [sync].destination to a local/OneDrive path",
        ));
    }
    export_to_path(paths, Path::new(&dest), opts)
}

/// Pull from configured destination (local path or HTTPS GET).
pub fn pull(paths: &WinzshPaths, cfg: &SyncConfig, opts: &ImportOptions) -> Result<ImportReport> {
    let dest = resolve_destination(cfg)?;
    import_from(paths, &dest, opts)
}

fn resolve_destination(cfg: &SyncConfig) -> Result<String> {
    if let Ok(env) = std::env::var("WINZSH_SYNC_DEST") {
        let t = env.trim();
        if !t.is_empty() {
            return Ok(t.to_string());
        }
    }
    let d = cfg.destination.trim();
    if d.is_empty() {
        return Err(message(
            "no sync destination — set [sync].destination, WINZSH_SYNC_DEST, or pass --path",
        ));
    }
    Ok(d.to_string())
}

fn sanitize_config(mut cfg: Config) -> Config {
    // Machine-local; do not roam checkout paths.
    cfg.update.source_dir.clear();
    // Telemetry stays off when roaming unless user re-enables.
    cfg.telemetry.enabled = false;
    cfg
}

fn collect_plugin_files(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    collect_files_rec(root, root, &mut out)?;
    out.remove(".winzsh-origin.toml");
    Ok(out)
}

fn collect_files_rec(
    root: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, String>,
) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|source| winzsh_error::io(dir.to_path_buf(), source))?;
    for entry in entries {
        let entry = entry.map_err(|source| winzsh_error::io(dir.to_path_buf(), source))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_rec(root, &path, out)?;
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|_| message("plugin path prefix error"))?
            .to_string_lossy()
            .replace('\\', "/");
        // Skip obvious binaries.
        if rel.ends_with(".exe") || rel.ends_with(".dll") {
            continue;
        }
        match read_string(&path) {
            Ok(text) => {
                out.insert(rel, text);
            }
            Err(_) => {
                // skip non-utf8
            }
        }
    }
    Ok(())
}

fn resolve_bundle_file(dest: &Path, for_write: bool) -> Result<PathBuf> {
    if dest
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        if for_write {
            if let Some(parent) = dest.parent() {
                ensure_dir(parent)?;
            }
        }
        return Ok(dest.to_path_buf());
    }
    if for_write {
        ensure_dir(dest)?;
    } else if dest.is_dir() {
        let candidate = dest.join("winzsh-sync.json");
        if candidate.is_file() {
            return Ok(candidate);
        }
        return Err(message(format!(
            "no winzsh-sync.json in {}",
            dest.display()
        )));
    }
    if dest.is_dir() || for_write {
        return Ok(dest.join("winzsh-sync.json"));
    }
    Ok(dest.to_path_buf())
}

fn read_bundle_bytes(source: &str) -> Result<(String, Vec<u8>)> {
    let source = source.trim();
    if source.starts_with("https://") || source.starts_with("http://") {
        let resp = ureq::get(source)
            .set("User-Agent", &format!("winzsh/{VERSION}"))
            .timeout(std::time::Duration::from_secs(60))
            .call()
            .map_err(|e| message(format!("sync pull failed: {e}")))?;
        if !(200..300).contains(&resp.status()) {
            return Err(message(format!("sync pull HTTP {}", resp.status())));
        }
        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| message(format!("sync pull read: {e}")))?;
        return Ok((source.to_string(), bytes));
    }
    let path = resolve_bundle_file(Path::new(source), false)?;
    let bytes = fs::read(&path).map_err(|source| winzsh_error::io(path.clone(), source))?;
    Ok((path.display().to_string(), bytes))
}

fn load_state(paths: &WinzshPaths) -> SyncState {
    let path = paths.sync_state_file();
    if !path.is_file() {
        return SyncState::default();
    }
    read_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save_state(paths: &WinzshPaths, state: &SyncState) -> Result<()> {
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| message(format!("serialize sync state: {e}")))?;
    atomic_write(&paths.sync_state_file(), json)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use winzsh_test_support::TempHome;

    #[test]
    fn export_import_roundtrip() {
        let home = TempHome::new("sync");
        let paths = &home.paths;
        winzsh_fs::ensure_layout(paths).expect("layout");
        let mut cfg = Config {
            theme: "tokyo-night".into(),
            ..Config::default()
        };
        cfg.aliases.insert("ll".into(), "Get-ChildItem".into());
        cfg.plugins.enabled.push("git".into());
        config::save(paths, &cfg).expect("save");
        let state = State::new_install(cfg.schema_version);
        atomic_write(&paths.state_file(), state.to_json().expect("json")).expect("state");

        let export_dir = home.root.join("bundle-out");
        let report = export_to_path(
            paths,
            &export_dir,
            &ExportOptions {
                include_plugins: false,
                include_history: false,
                history_limit: 10,
            },
        )
        .expect("export");
        assert!(Path::new(&report.destination).is_file());

        // Change theme then import.
        cfg.theme = "minimal".into();
        config::save(paths, &cfg).expect("save2");

        let imported = import_from(
            paths,
            &report.destination,
            &ImportOptions {
                force: true,
                prefer_bundled_plugins: true,
            },
        )
        .expect("import");
        assert_eq!(imported.theme, "tokyo-night");
        let loaded = config::load(paths).expect("load");
        assert_eq!(loaded.theme, "tokyo-night");
        assert_eq!(loaded.aliases.get("ll").map(String::as_str), Some("Get-ChildItem"));
        let _ = fs::remove_dir_all(&home.paths.root);
    }
}
