//! History store schema, spool compaction, and query API.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::Path;
use time::OffsetDateTime;
use tracing::debug;
use winzsh_core::WinzshPaths;
use winzsh_error::{Result, message};
use winzsh_fs::{append_string, atomic_write, ensure_dir, read_string};

/// One recorded command invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Command line text.
    pub command: String,
    /// Working directory when the command ran.
    #[serde(default)]
    pub cwd: String,
    /// Shell name (`pwsh`, `powershell`, …).
    #[serde(default)]
    pub shell: String,
    /// RFC3339 timestamp.
    pub timestamp: String,
    /// Exit code when known.
    #[serde(default)]
    pub exit_code: Option<i32>,
    /// Duration in milliseconds when known.
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// Query options for listing history.
#[derive(Debug, Clone)]
pub struct HistoryQuery {
    /// Maximum entries to return (most recent first).
    pub limit: usize,
    /// Optional substring filter (case-insensitive).
    pub contains: Option<String>,
}

impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            limit: 50,
            contains: None,
        }
    }
}

/// Render PowerShell history spool helpers.
pub fn render_powershell(paths: &WinzshPaths, enabled: bool) -> String {
    let spool = paths.history_spool();
    let spool_ps = spool.display().to_string().replace('\'', "''");
    let enabled_ps = if enabled { "$true" } else { "$false" };
    format!(
        r#"
# --- history (phase 2) ---
$script:WinZshHistoryEnabled = {enabled_ps}
$script:WinZshHistorySpool = '{spool_ps}'
$script:WinZshLastHistoryId = 0

function Write-WinZshHistoryFromPrompt {{
    [CmdletBinding()]
    param()
    if (-not $script:WinZshHistoryEnabled) {{ return }}
    try {{
        $h = Get-History -Count 1 -ErrorAction SilentlyContinue
        if (-not $h) {{ return }}
        if ($h.Id -eq $script:WinZshLastHistoryId) {{ return }}
        $script:WinZshLastHistoryId = $h.Id
        $dir = Split-Path -Parent $script:WinZshHistorySpool
        if (-not (Test-Path -LiteralPath $dir)) {{
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }}
        $entry = [ordered]@{{
            command = [string]$h.CommandLine
            cwd = (Get-Location).Path
            shell = if ($PSVersionTable.PSEdition -eq 'Core') {{ 'pwsh' }} else {{ 'powershell' }}
            timestamp = (Get-Date).ToUniversalTime().ToString('o')
            exit_code = $null
            duration_ms = $null
        }}
        ($entry | ConvertTo-Json -Compress) | Add-Content -LiteralPath $script:WinZshHistorySpool -Encoding utf8
    }} catch {{
        Write-Verbose "WinZSH history write failed: $_"
    }}
}}
"#
    )
}

/// Compact the spool into the history store and optionally trim.
pub fn compact(paths: &WinzshPaths, max_entries: usize) -> Result<usize> {
    ensure_dir(&paths.history_dir())?;
    let mut entries = load_all(paths)?;
    let before = entries.len();
    if max_entries > 0 && entries.len() > max_entries {
        let skip = entries.len() - max_entries;
        entries = entries.split_off(skip);
    }
    write_store(paths, &entries)?;
    if paths.history_spool().is_file() {
        atomic_write(&paths.history_spool(), "")?;
    }
    debug!(before, after = entries.len(), "compacted history");
    Ok(entries.len())
}

/// List history entries (most recent first).
pub fn query(paths: &WinzshPaths, q: &HistoryQuery) -> Result<Vec<HistoryEntry>> {
    let mut entries = load_all(paths)?;
    if let Some(filter) = &q.contains {
        let needle = filter.to_ascii_lowercase();
        entries.retain(|e| e.command.to_ascii_lowercase().contains(&needle));
    }
    entries.reverse();
    if q.limit > 0 && entries.len() > q.limit {
        entries.truncate(q.limit);
    }
    Ok(entries)
}

/// Append a single entry to the store (tests / tooling).
pub fn append(paths: &WinzshPaths, entry: &HistoryEntry) -> Result<()> {
    ensure_dir(&paths.history_dir())?;
    let line = serde_json::to_string(entry)
        .map_err(|e| message(format!("serialize history entry: {e}")))?;
    append_string(&paths.history_store(), &format!("{line}\n"))
}

fn load_all(paths: &WinzshPaths) -> Result<Vec<HistoryEntry>> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    read_jsonl(&paths.history_store(), &mut entries, &mut seen)?;
    read_jsonl(&paths.history_spool(), &mut entries, &mut seen)?;
    Ok(entries)
}

fn read_jsonl(
    path: &Path,
    entries: &mut Vec<HistoryEntry>,
    seen: &mut HashSet<String>,
) -> Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let file = std::fs::File::open(path).map_err(|source| winzsh_error::io(path, source))?;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|source| winzsh_error::io(path, source))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<HistoryEntry>(line) {
            Ok(entry) => {
                let key = format!("{}|{}|{}", entry.timestamp, entry.cwd, entry.command);
                if seen.insert(key) {
                    entries.push(entry);
                }
            }
            Err(err) => {
                debug!(%err, "skip malformed history line");
            }
        }
    }
    Ok(())
}

fn write_store(paths: &WinzshPaths, entries: &[HistoryEntry]) -> Result<()> {
    let mut body = String::new();
    for entry in entries {
        let line = serde_json::to_string(entry)
            .map_err(|e| message(format!("serialize history entry: {e}")))?;
        body.push_str(&line);
        body.push('\n');
    }
    atomic_write(&paths.history_store(), body)
}

/// Create a timestamped entry for the current moment.
pub fn entry_now(
    command: impl Into<String>,
    cwd: impl Into<String>,
    shell: impl Into<String>,
) -> HistoryEntry {
    let timestamp = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into());
    HistoryEntry {
        command: command.into(),
        cwd: cwd.into(),
        shell: shell.into(),
        timestamp,
        exit_code: None,
        duration_ms: None,
    }
}

/// Ensure history files exist (empty ok).
pub fn ensure(paths: &WinzshPaths) -> Result<()> {
    ensure_dir(&paths.history_dir())?;
    if !paths.history_store().is_file() {
        atomic_write(&paths.history_store(), "")?;
    }
    if !paths.history_spool().is_file() {
        atomic_write(&paths.history_spool(), "")?;
    }
    // Touch read to validate permissions.
    let _ = read_string(&paths.history_store())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use winzsh_fs::ensure_layout;

    #[test]
    fn append_and_query() {
        let root = std::env::temp_dir().join(format!("winzsh-hist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = WinzshPaths::from_root(root.clone());
        ensure_layout(&paths).expect("layout");
        ensure(&paths).expect("ensure");
        append(&paths, &entry_now("git status", "C:\\repo", "pwsh")).expect("append");
        let items = query(
            &paths,
            &HistoryQuery {
                limit: 10,
                contains: Some("git".into()),
            },
        )
        .expect("query");
        assert_eq!(items.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
