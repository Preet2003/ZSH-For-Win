//! Remote plugin registry: index fetch/cache, checksum verify, zip install.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use tracing::info;
use winzsh_config::RegistryConfig;
use winzsh_core::{VERSION, WinzshPaths};
use winzsh_error::{Result, message};
use winzsh_fs::{atomic_write, ensure_dir};
use winzsh_plugin::{self as plugin, PluginManifest};

/// Bundled community packages shipped with the CLI (offline-capable).
const EMBEDDED_PACKAGES: &[(&str, &[u8])] = &[(
    "demo-aliases",
    include_bytes!("../../../registry/packages/demo-aliases-0.1.0.zip"),
)];

const EMBEDDED_INDEX: &str = include_str!("../../../registry/index.json");

/// Default public index URL.
pub const DEFAULT_INDEX_URL: &str =
    "https://raw.githubusercontent.com/winzsh/winzsh/main/registry/index.json";

/// Registry index document (schema_version = 1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryIndex {
    /// Schema version.
    pub schema_version: u32,
    /// ISO-ish timestamp.
    #[serde(default)]
    pub updated_at: String,
    /// Plugin entries.
    #[serde(default)]
    pub plugins: Vec<RegistryPlugin>,
}

/// One plugin listed in the registry index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryPlugin {
    /// Stable id (must match plugin.toml `name`).
    pub id: String,
    /// Published version.
    pub version: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Author / publisher label.
    #[serde(default)]
    pub author: String,
    /// Search tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// `https://…zip`, `file:///…`, `path:relative`, or `embedded:id`.
    pub download_url: String,
    /// SHA-256 hex of the zip (or of folder archive bytes for embedded).
    pub sha256: String,
    /// Optional signature (minisign/ed25519) — required when config says so.
    #[serde(default)]
    pub signature: Option<String>,
    /// Optional homepage.
    #[serde(default)]
    pub homepage: String,
    /// Optional relative package path (docs / tooling).
    #[serde(default)]
    pub package: String,
}

/// Where an index was loaded from (for resolving `path:` URLs).
#[derive(Debug, Clone)]
pub enum IndexOrigin {
    /// Remote HTTPS index.
    Url(String),
    /// Local filesystem index.
    File(PathBuf),
    /// Compiled-in fallback.
    Embedded,
}

/// Loaded index + origin metadata.
#[derive(Debug, Clone)]
pub struct LoadedIndex {
    /// Parsed index.
    pub index: RegistryIndex,
    /// Origin used for relative `path:` resolution.
    pub origin: IndexOrigin,
    /// Whether this came from network / cache / embedded.
    pub source: String,
}

/// Fetch (or cache / embed) the plugin index.
pub fn fetch_index(paths: &WinzshPaths, cfg: &RegistryConfig) -> Result<LoadedIndex> {
    let url = resolve_index_url(cfg);
    ensure_dir(&paths.registry_cache_dir())?;

    if let Some(path) = file_url_to_path(&url) {
        let raw =
            fs::read_to_string(&path).map_err(|source| winzsh_error::io(path.clone(), source))?;
        let index = parse_index(&raw)?;
        atomic_write(&paths.registry_index_cache(), &raw)?;
        return Ok(LoadedIndex {
            index,
            origin: IndexOrigin::File(path),
            source: "file".into(),
        });
    }

    match http_get_text(&url) {
        Ok(raw) => {
            let index = parse_index(&raw)?;
            atomic_write(&paths.registry_index_cache(), &raw)?;
            Ok(LoadedIndex {
                index,
                origin: IndexOrigin::Url(url),
                source: "network".into(),
            })
        }
        Err(net_err) => {
            if paths.registry_index_cache().is_file() {
                let raw = winzsh_fs::read_string(&paths.registry_index_cache())?;
                let index = parse_index(&raw)?;
                info!(error = %net_err, "registry network failed; using cache");
                return Ok(LoadedIndex {
                    index,
                    origin: IndexOrigin::Url(url),
                    source: "cache".into(),
                });
            }
            let index = parse_index(EMBEDDED_INDEX)?;
            info!(error = %net_err, "registry network failed; using embedded index");
            Ok(LoadedIndex {
                index,
                origin: IndexOrigin::Embedded,
                source: "embedded".into(),
            })
        }
    }
}

/// Search plugins by id/description/tags substring (case-insensitive).
pub fn search(index: &RegistryIndex, query: &str) -> Vec<RegistryPlugin> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return index.plugins.clone();
    }
    index
        .plugins
        .iter()
        .filter(|p| {
            p.id.to_ascii_lowercase().contains(&q)
                || p.description.to_ascii_lowercase().contains(&q)
                || p.author.to_ascii_lowercase().contains(&q)
                || p.tags.iter().any(|t| t.to_ascii_lowercase().contains(&q))
        })
        .cloned()
        .collect()
}

/// Look up a single plugin id.
pub fn find<'a>(index: &'a RegistryIndex, id: &str) -> Option<&'a RegistryPlugin> {
    index.plugins.iter().find(|p| p.id == id)
}

/// Install a registry plugin into `~/.winzsh/plugins/<id>`.
pub fn install(
    paths: &WinzshPaths,
    cfg: &RegistryConfig,
    loaded: &LoadedIndex,
    id: &str,
    overwrite: bool,
) -> Result<PluginManifest> {
    let entry = find(&loaded.index, id).ok_or_else(|| {
        message(format!(
            "plugin '{id}' not found in registry (try `winzsh plugin search`)"
        ))
    })?;
    install_entry(paths, cfg, loaded, entry, overwrite)
}

fn install_entry(
    paths: &WinzshPaths,
    cfg: &RegistryConfig,
    loaded: &LoadedIndex,
    entry: &RegistryPlugin,
    overwrite: bool,
) -> Result<PluginManifest> {
    if cfg.require_signature
        && entry
            .signature
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    {
        return Err(message(format!(
            "registry plugin '{}' has no signature and registry.require_signature=true",
            entry.id
        )));
    }

    let bytes = download_package(paths, loaded, entry)?;
    let actual = sha256_hex(&bytes);
    let expected = entry.sha256.trim().to_ascii_lowercase();
    if expected.is_empty() {
        return Err(message(format!(
            "registry entry '{}' is missing sha256",
            entry.id
        )));
    }
    if actual != expected {
        return Err(message(format!(
            "checksum mismatch for '{}': expected {expected}, got {actual}",
            entry.id
        )));
    }

    let staging = paths.registry_cache_dir().join("staging").join(format!(
        "{}-{}",
        entry.id,
        std::process::id()
    ));
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    ensure_dir(&staging)?;
    extract_zip(&bytes, &staging)?;

    let plugin_root = find_plugin_root(&staging)?;
    let manifest = plugin::install_from_dir(paths, &plugin_root, overwrite)?;
    if manifest.name != entry.id {
        let _ = plugin::remove(paths, &manifest.name);
        return Err(message(format!(
            "registry id '{}' does not match plugin.toml name '{}'",
            entry.id, manifest.name
        )));
    }
    plugin::write_origin(paths, &entry.id, "registry", &entry.version, Some(&actual))?;
    let _ = fs::remove_dir_all(&staging);
    info!(plugin = %entry.id, version = %entry.version, "installed from registry");
    Ok(manifest)
}

/// Update one plugin (or all registry-origin plugins) from the index.
pub fn update(
    paths: &WinzshPaths,
    cfg: &RegistryConfig,
    loaded: &LoadedIndex,
    only_id: Option<&str>,
) -> Result<Vec<PluginManifest>> {
    let mut updated = Vec::new();
    let targets: Vec<RegistryPlugin> = if let Some(id) = only_id {
        vec![
            find(&loaded.index, id)
                .cloned()
                .ok_or_else(|| message(format!("plugin '{id}' not in registry")))?,
        ]
    } else {
        loaded.index.plugins.clone()
    };

    for entry in targets {
        let installed = match plugin::load(paths, &entry.id) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let origin = plugin::read_origin_source(paths, &entry.id).unwrap_or_default();
        if only_id.is_none() && origin != "registry" {
            continue;
        }
        if installed.manifest.version == entry.version && only_id.is_none() {
            continue;
        }
        let manifest = install_entry(paths, cfg, loaded, &entry, true)?;
        updated.push(manifest);
    }
    Ok(updated)
}

fn resolve_index_url(cfg: &RegistryConfig) -> String {
    if let Ok(env) = std::env::var("WINZSH_REGISTRY_URL") {
        let t = env.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    let configured = cfg.url.trim();
    if configured.is_empty() {
        DEFAULT_INDEX_URL.to_string()
    } else {
        configured.to_string()
    }
}

fn parse_index(raw: &str) -> Result<RegistryIndex> {
    let index: RegistryIndex =
        serde_json::from_str(raw).map_err(|e| message(format!("registry index parse: {e}")))?;
    if index.schema_version != 1 {
        return Err(message(format!(
            "unsupported registry schema_version {}",
            index.schema_version
        )));
    }
    Ok(index)
}

fn http_get_text(url: &str) -> Result<String> {
    let resp = ureq::get(url)
        .set("User-Agent", &format!("winzsh/{VERSION}"))
        .set("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| message(format!("registry fetch failed: {e}")))?;
    if !(200..300).contains(&resp.status()) {
        return Err(message(format!("registry HTTP {}", resp.status())));
    }
    resp.into_string()
        .map_err(|e| message(format!("registry read body: {e}")))
}

fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    let resp = ureq::get(url)
        .set("User-Agent", &format!("winzsh/{VERSION}"))
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| message(format!("download failed: {e}")))?;
    if !(200..300).contains(&resp.status()) {
        return Err(message(format!("download HTTP {}", resp.status())));
    }
    let mut reader = resp.into_reader();
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| message(format!("download read: {e}")))?;
    Ok(bytes)
}

fn download_package(
    paths: &WinzshPaths,
    loaded: &LoadedIndex,
    entry: &RegistryPlugin,
) -> Result<Vec<u8>> {
    let url = entry.download_url.trim();
    if let Some(id) = url.strip_prefix("embedded:") {
        return embedded_package(id.trim());
    }
    if let Some(rel) = url.strip_prefix("path:") {
        let base = match &loaded.origin {
            IndexOrigin::File(p) => p
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| message("index path has no parent"))?,
            IndexOrigin::Url(_) | IndexOrigin::Embedded => {
                // Fall back to package field next to embedded index layout via cache.
                return Err(message(
                    "path: download_url only works with a file:// registry index",
                ));
            }
        };
        let target = base.join(rel.trim());
        if target.is_dir() {
            return zip_directory(&target);
        }
        return fs::read(&target).map_err(|source| winzsh_error::io(target, source));
    }
    if let Some(path) = file_url_to_path(url) {
        return fs::read(&path).map_err(|source| winzsh_error::io(path, source));
    }
    if url.starts_with("https://") || url.starts_with("http://") {
        let bytes = http_get_bytes(url)?;
        let cache_file = paths
            .registry_cache_dir()
            .join("downloads")
            .join(format!("{}-{}.zip", entry.id, entry.version));
        ensure_dir(cache_file.parent().unwrap_or(Path::new(".")))?;
        let _ = atomic_write(&cache_file, &bytes);
        return Ok(bytes);
    }
    Err(message(format!(
        "unsupported download_url for '{}': {url}",
        entry.id
    )))
}

fn embedded_package(id: &str) -> Result<Vec<u8>> {
    EMBEDDED_PACKAGES
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, bytes)| bytes.to_vec())
        .ok_or_else(|| message(format!("no embedded package '{id}' in this CLI build")))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn extract_zip(bytes: &[u8], dest: &Path) -> Result<()> {
    let reader = Cursor::new(bytes);
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| message(format!("invalid plugin zip: {e}")))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| message(format!("zip entry: {e}")))?;
        let Some(rel) = file.enclosed_name() else {
            continue;
        };
        let rel = rel.to_path_buf();
        if rel.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(message(format!("zip slip blocked: {}", rel.display())));
        }
        let out = dest.join(&rel);
        if file.is_dir() {
            ensure_dir(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            ensure_dir(parent)?;
        }
        let mut outfile =
            fs::File::create(&out).map_err(|source| winzsh_error::io(out.clone(), source))?;
        std::io::copy(&mut file, &mut outfile)
            .map_err(|source| winzsh_error::io(out.clone(), source))?;
    }
    Ok(())
}

fn find_plugin_root(staging: &Path) -> Result<PathBuf> {
    if staging.join("plugin.toml").is_file() {
        return Ok(staging.to_path_buf());
    }
    let entries =
        fs::read_dir(staging).map_err(|source| winzsh_error::io(staging.to_path_buf(), source))?;
    for entry in entries {
        let entry = entry.map_err(|source| winzsh_error::io(staging.to_path_buf(), source))?;
        let path = entry.path();
        if path.is_dir() && path.join("plugin.toml").is_file() {
            return Ok(path);
        }
    }
    Err(message(format!(
        "extracted package has no plugin.toml under {}",
        staging.display()
    )))
}

/// Zip a directory (plugin.toml at archive root) — used for path: folder installs.
fn zip_directory(dir: &Path) -> Result<Vec<u8>> {
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        add_dir_to_zip(&mut zip, dir, Path::new(""), options)?;
        zip.finish()
            .map_err(|e| message(format!("zip finish: {e}")))?;
    }
    Ok(cursor.into_inner())
}

fn add_dir_to_zip<W: Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    dir: &Path,
    prefix: &Path,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    let entries =
        fs::read_dir(dir).map_err(|source| winzsh_error::io(dir.to_path_buf(), source))?;
    for entry in entries {
        let entry = entry.map_err(|source| winzsh_error::io(dir.to_path_buf(), source))?;
        let path = entry.path();
        let name = entry.file_name();
        let rel = prefix.join(&name);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if path.is_dir() {
            zip.add_directory(format!("{rel_str}/"), options)
                .map_err(|e| message(format!("zip dir: {e}")))?;
            add_dir_to_zip(zip, &path, &rel, options)?;
        } else {
            zip.start_file(&rel_str, options)
                .map_err(|e| message(format!("zip file: {e}")))?;
            let bytes = fs::read(&path).map_err(|source| winzsh_error::io(path.clone(), source))?;
            zip.write_all(&bytes)
                .map_err(|e| message(format!("zip write: {e}")))?;
        }
    }
    Ok(())
}

fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let url = url.trim();
    let rest = url.strip_prefix("file://")?;
    // file:///C:/path or file:/C:/path or file:///home/...
    let path = if cfg!(windows) {
        let trimmed = rest.trim_start_matches('/');
        if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
            PathBuf::from(trimmed)
        } else {
            PathBuf::from(rest)
        }
    } else {
        PathBuf::from(rest)
    };
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use winzsh_test_support::TempHome;

    #[test]
    fn parses_embedded_index() {
        let index = parse_index(EMBEDDED_INDEX).expect("index");
        assert_eq!(index.schema_version, 1);
        assert!(find(&index, "demo-aliases").is_some());
    }

    #[test]
    fn search_filters() {
        let index = parse_index(EMBEDDED_INDEX).expect("index");
        let hits = search(&index, "demo");
        assert_eq!(hits.len(), 1);
        assert!(search(&index, "nope-not-real").is_empty());
    }

    #[test]
    fn install_embedded_demo() {
        let home = TempHome::new("registry-install");
        let cfg = RegistryConfig::default();
        let loaded = LoadedIndex {
            index: parse_index(EMBEDDED_INDEX).expect("index"),
            origin: IndexOrigin::Embedded,
            source: "embedded".into(),
        };
        let manifest = install(&home.paths, &cfg, &loaded, "demo-aliases", false).expect("install");
        assert_eq!(manifest.name, "demo-aliases");
        assert!(
            home.paths
                .plugins_dir()
                .join("demo-aliases/plugin.toml")
                .is_file()
        );
        assert_eq!(
            plugin::read_origin_source(&home.paths, "demo-aliases").as_deref(),
            Some("registry")
        );
        let _ = fs::remove_dir_all(&home.paths.root);
    }
}
