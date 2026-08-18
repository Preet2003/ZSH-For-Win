//! Self-update: GitHub Releases check/apply, from-source rebuild, rollback.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;
use winzsh_config::UpdateConfig;
use winzsh_core::{Channel, VERSION, WinzshPaths};
use winzsh_error::{Result, message};
use winzsh_fs::{atomic_write, ensure_dir};

/// Outcome of an update check (no binary changes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckReport {
    /// Currently running / installed CLI version.
    pub current_version: String,
    /// Newest version found (if any).
    pub latest_version: Option<String>,
    /// Whether an update is available.
    pub update_available: bool,
    /// Channel used for the check.
    pub channel: String,
    /// Source of the latest version (`github`, `source`, or `none`).
    pub source: String,
    /// Human notes (missing repo, etc.).
    pub notes: Vec<String>,
    /// Download URL when from GitHub.
    pub download_url: Option<String>,
}

/// Outcome of applying an update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplyReport {
    /// Previous version (backup retained when possible).
    pub previous_version: String,
    /// Version now installed.
    pub installed_version: String,
    /// How the update was applied.
    pub method: String,
    /// Path to the active CLI binary.
    pub binary: String,
    /// Steps performed.
    pub steps: Vec<String>,
}

/// Options for `update --from-source`.
#[derive(Debug, Clone, Default)]
pub struct FromSourceOptions {
    /// Git checkout root that contains `Cargo.toml` (workspace or package).
    pub source_dir: PathBuf,
    /// Run `git pull --ff-only` before building.
    pub pull: bool,
}

/// Check for a newer GitHub Release (or report that only from-source is available).
pub fn check(paths: &WinzshPaths, cfg: &UpdateConfig) -> Result<CheckReport> {
    let current = installed_or_running_version(paths);
    let channel = match cfg.channel {
        Channel::Stable => "stable",
        Channel::Beta => "beta",
    }
    .to_string();

    let repo = cfg.github_repo.trim();
    if repo.is_empty() {
        return Ok(CheckReport {
            current_version: current,
            latest_version: None,
            update_available: false,
            channel,
            source: "none".into(),
            notes: vec![
                "update.github_repo is empty — GitHub Releases check skipped".into(),
                "Use: winzsh update --from-source [path]  (rebuild from a git checkout)".into(),
                "Or set [update] github_repo = \"owner/repo\" in config.toml".into(),
            ],
            download_url: None,
        });
    }

    let release = fetch_release(repo, cfg.channel)?;
    let latest = strip_v_prefix(&release.tag_name);
    let newer = version_gt(&latest, &current);
    let asset = pick_windows_asset(&release.assets);
    let mut notes = Vec::new();
    if asset.is_none() {
        notes.push(
            "No Windows .exe asset found on the latest release (expected name containing winzsh and .exe)"
                .into(),
        );
    }

    Ok(CheckReport {
        current_version: current,
        latest_version: Some(latest.clone()),
        update_available: newer && asset.is_some(),
        channel,
        source: "github".into(),
        notes,
        download_url: asset.map(|a| a.browser_download_url),
    })
}

/// Download + replace the installed CLI from GitHub Releases.
pub fn apply_github(paths: &WinzshPaths, cfg: &UpdateConfig) -> Result<ApplyReport> {
    let report = check(paths, cfg)?;
    if !report.update_available {
        return Err(message(format!(
            "no GitHub update to apply (current {}, latest {:?}). {}",
            report.current_version,
            report.latest_version,
            report.notes.join(" ")
        )));
    }
    let url = report
        .download_url
        .ok_or_else(|| message("missing download URL"))?;
    let latest = report
        .latest_version
        .ok_or_else(|| message("missing latest version"))?;

    let mut steps = Vec::new();
    ensure_dir(&paths.bin_dir())?;
    let staged = paths.bin_dir().join("winzsh.exe.new");
    download_file(&url, &staged)?;
    steps.push(format!("downloaded {url}"));

    let previous = report.current_version.clone();
    replace_cli_binary(paths, &staged)?;
    steps.push(format!("installed {}", paths.cli_binary().display()));
    steps.push(format!(
        "previous binary backed up to {}",
        paths.cli_binary_backup().display()
    ));

    info!(from = %previous, to = %latest, "applied GitHub self-update");
    Ok(ApplyReport {
        previous_version: previous,
        installed_version: latest,
        method: "github".into(),
        binary: paths.cli_binary().display().to_string(),
        steps,
    })
}

/// Rebuild from a local git/cargo checkout and replace `~/.winzsh/bin/winzsh.exe`.
pub fn apply_from_source(paths: &WinzshPaths, opts: &FromSourceOptions) -> Result<ApplyReport> {
    let source = opts
        .source_dir
        .canonicalize()
        .map_err(|source| winzsh_error::io(opts.source_dir.clone(), source))?;
    if !source.join("Cargo.toml").is_file() {
        return Err(message(format!(
            "no Cargo.toml in {} — pass the zsh-for-win workspace root",
            source.display()
        )));
    }

    let mut steps = Vec::new();
    let previous = installed_or_running_version(paths);

    if opts.pull {
        let status = Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(&source)
            .status()
            .map_err(|e| message(format!("git pull failed to start: {e}")))?;
        if !status.success() {
            return Err(message(format!("git pull --ff-only failed ({status})")));
        }
        steps.push("git pull --ff-only".into());
    }

    let status = Command::new("cargo")
        .args(["build", "-p", "winzsh", "--release"])
        .current_dir(&source)
        .status()
        .map_err(|e| message(format!("cargo build failed to start: {e}")))?;
    if !status.success() {
        return Err(message(format!(
            "cargo build -p winzsh --release failed ({status})"
        )));
    }
    steps.push("cargo build -p winzsh --release".into());

    let built = source
        .join("target")
        .join("release")
        .join(if cfg!(windows) {
            "winzsh.exe"
        } else {
            "winzsh"
        });
    if !built.is_file() {
        return Err(message(format!(
            "build finished but binary missing at {}",
            built.display()
        )));
    }

    ensure_dir(&paths.bin_dir())?;
    let staged = paths.bin_dir().join("winzsh.exe.new");
    fs::copy(&built, &staged).map_err(|source| winzsh_error::io(staged.clone(), source))?;
    steps.push(format!("staged {}", staged.display()));

    replace_cli_binary(paths, &staged)?;
    steps.push(format!("installed {}", paths.cli_binary().display()));

    let installed = read_binary_version(paths).unwrap_or_else(|| VERSION.to_string());
    info!(from = %previous, to = %installed, "applied from-source update");
    Ok(ApplyReport {
        previous_version: previous,
        installed_version: installed,
        method: "from-source".into(),
        binary: paths.cli_binary().display().to_string(),
        steps,
    })
}

/// Restore `winzsh.exe.bak` over the current CLI.
pub fn rollback(paths: &WinzshPaths) -> Result<ApplyReport> {
    let bak = paths.cli_binary_backup();
    if !bak.is_file() {
        return Err(message(format!(
            "no backup at {} — nothing to roll back",
            bak.display()
        )));
    }
    let previous = installed_or_running_version(paths);
    let staged = paths.bin_dir().join("winzsh.exe.new");
    fs::copy(&bak, &staged).map_err(|source| winzsh_error::io(staged.clone(), source))?;
    replace_cli_binary(paths, &staged)?;
    let installed = read_binary_version(paths).unwrap_or_else(|| "unknown".into());
    Ok(ApplyReport {
        previous_version: previous,
        installed_version: installed,
        method: "rollback".into(),
        binary: paths.cli_binary().display().to_string(),
        steps: vec![format!("restored from {}", bak.display())],
    })
}

/// Resolve source dir from flag, config, env, or cwd.
pub fn resolve_source_dir(explicit: Option<PathBuf>, cfg: &UpdateConfig) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    if let Ok(env) = std::env::var("WINZSH_SOURCE") {
        let t = env.trim();
        if !t.is_empty() {
            return Ok(PathBuf::from(t));
        }
    }
    let configured = cfg.source_dir.trim();
    if !configured.is_empty() {
        return Ok(PathBuf::from(configured));
    }
    // Walk cwd upward looking for workspace Cargo.toml that lists winzsh.
    let mut dir = std::env::current_dir().map_err(|e| message(format!("cwd: {e}")))?;
    loop {
        let cargo = dir.join("Cargo.toml");
        if cargo.is_file()
            && let Ok(raw) = fs::read_to_string(&cargo)
            && (raw.contains("name = \"winzsh\"") || raw.contains("crates/winzsh"))
        {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    Err(message(
        "could not find source checkout — pass --from-source <path>, set WINZSH_SOURCE, or [update].source_dir",
    ))
}

fn installed_or_running_version(paths: &WinzshPaths) -> String {
    read_binary_version(paths).unwrap_or_else(|| VERSION.to_string())
}

fn read_binary_version(paths: &WinzshPaths) -> Option<String> {
    let bin = paths.cli_binary();
    if !bin.is_file() {
        return None;
    }
    let out = Command::new(&bin).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // "winzsh 0.1.0" or "winzsh.exe 0.1.0"
    text.split_whitespace().nth(1).map(|s| s.trim().to_string())
}

/// Replace installed CLI using Windows-friendly rename of the running file.
fn replace_cli_binary(paths: &WinzshPaths, new_file: &Path) -> Result<()> {
    ensure_dir(&paths.bin_dir())?;
    let dest = paths.cli_binary();
    let bak = paths.cli_binary_backup();

    if dest.is_file() {
        let _ = fs::remove_file(&bak);
        fs::rename(&dest, &bak).map_err(|source| winzsh_error::io(bak.clone(), source))?;
    }
    fs::rename(new_file, &dest).or_else(|_| {
        fs::copy(new_file, &dest)
            .map(|_| ())
            .map_err(|source| winzsh_error::io(dest.clone(), source))
    })?;
    let _ = fs::remove_file(new_file);
    Ok(())
}

fn download_file(url: &str, dest: &Path) -> Result<()> {
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
    std::io::copy(&mut reader, &mut bytes)
        .map_err(|e| message(format!("download read failed: {e}")))?;
    if bytes.len() < 1024 {
        return Err(message(
            "downloaded file looks too small to be a CLI binary",
        ));
    }
    atomic_write(dest, &bytes)?;
    Ok(())
}

#[derive(Debug, Deserialize, Clone)]
struct GhRelease {
    tag_name: String,
    prerelease: bool,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize, Clone)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn fetch_release(repo: &str, channel: Channel) -> Result<GhRelease> {
    let url = match channel {
        Channel::Stable => format!("https://api.github.com/repos/{repo}/releases/latest"),
        Channel::Beta => format!("https://api.github.com/repos/{repo}/releases?per_page=10"),
    };
    let resp = ureq::get(&url)
        .set("User-Agent", &format!("winzsh/{VERSION}"))
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| message(format!("GitHub API error: {e}")))?;
    if resp.status() == 404 {
        return Err(message(format!(
            "GitHub repo '{repo}' not found or has no releases yet"
        )));
    }
    if !(200..300).contains(&resp.status()) {
        return Err(message(format!("GitHub API HTTP {}", resp.status())));
    }

    match channel {
        Channel::Stable => resp
            .into_json::<GhRelease>()
            .map_err(|e| message(format!("parse release: {e}"))),
        Channel::Beta => {
            let list: Vec<GhRelease> = resp
                .into_json()
                .map_err(|e| message(format!("parse releases: {e}")))?;
            let preferred = list
                .iter()
                .find(|r| r.prerelease)
                .cloned()
                .or_else(|| list.first().cloned())
                .ok_or_else(|| message("no releases found on beta channel"))?;
            Ok(preferred)
        }
    }
}

fn pick_windows_asset(assets: &[GhAsset]) -> Option<GhAsset> {
    let mut scored: Vec<(i32, &GhAsset)> = assets
        .iter()
        .filter(|a| a.name.to_ascii_lowercase().ends_with(".exe"))
        .filter(|a| a.name.to_ascii_lowercase().contains("winzsh"))
        .filter(|a| !a.name.to_ascii_lowercase().contains("setup"))
        .map(|a| {
            let n = a.name.to_ascii_lowercase();
            let mut score = 0;
            if n.contains("x86_64") || n.contains("amd64") {
                score += 2;
            }
            if n.contains("windows") || n.contains("msvc") || n.contains("pc-windows") {
                score += 2;
            }
            if n == "winzsh.exe" {
                score += 3;
            }
            (score, a)
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.0));
    scored.into_iter().next().map(|(_, a)| a.clone())
}

fn strip_v_prefix(tag: &str) -> String {
    tag.trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

/// Return true if `a` is a greater semver-ish version than `b`.
pub fn version_gt(a: &str, b: &str) -> bool {
    parse_version(a) > parse_version(b)
}

fn parse_version(v: &str) -> (u64, u64, u64) {
    let clean = strip_v_prefix(v);
    let mut parts = clean.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|s| {
            let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().ok()
        })
        .unwrap_or(0);
    (major, minor, patch)
}

/// SHA-256 hex of a file (for future checksum verify).
pub fn file_sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| winzsh_error::io(path.to_path_buf(), source))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(version_gt("0.2.0", "0.1.9"));
        assert!(version_gt("v1.0.0", "0.9.9"));
        assert!(!version_gt("0.1.0", "0.1.0"));
        assert!(!version_gt("0.1.0", "0.2.0"));
    }

    #[test]
    fn strip_v() {
        assert_eq!(strip_v_prefix("v0.1.0"), "0.1.0");
    }

    #[test]
    fn pick_asset() {
        let assets = vec![
            GhAsset {
                name: "notes.txt".into(),
                browser_download_url: "http://x/notes".into(),
            },
            GhAsset {
                name: "winzsh-x86_64-pc-windows-msvc.exe".into(),
                browser_download_url: "http://x/winzsh.exe".into(),
            },
        ];
        let a = pick_windows_asset(&assets).expect("asset");
        assert!(a.name.contains("winzsh"));
    }
}
