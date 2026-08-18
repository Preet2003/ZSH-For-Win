//! Shell host abstraction for PowerShell today and additional shells (CMD / Nu / Git Bash).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use winzsh_error::Result;

mod bash;
mod cmd;
mod nu;

pub use bash::BashHost;
pub use cmd::CmdHost;
pub use nu::NuHost;

/// Stable shell identifiers used in config / CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellId {
    /// PowerShell 7 / Windows PowerShell (primary).
    PowerShell,
    /// Windows CMD via `zsh-for-win.cmd` launcher.
    Cmd,
    /// Nushell (`nu`) — experimental profile snippet.
    Nu,
    /// Git Bash / bash — experimental `.bashrc` snippet.
    Bash,
}

impl ShellId {
    /// Parse from CLI / config string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "powershell" | "pwsh" | "ps" => Some(Self::PowerShell),
            "cmd" | "command" | "cmd.exe" => Some(Self::Cmd),
            "nu" | "nushell" => Some(Self::Nu),
            "bash" | "git-bash" | "gitbash" => Some(Self::Bash),
            _ => None,
        }
    }

    /// Canonical config / CLI id.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PowerShell => "powershell",
            Self::Cmd => "cmd",
            Self::Nu => "nu",
            Self::Bash => "bash",
        }
    }

    /// All known shells.
    pub fn all() -> &'static [ShellId] {
        &[Self::PowerShell, Self::Cmd, Self::Nu, Self::Bash]
    }
}

impl std::fmt::Display for ShellId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Capabilities advertised by a shell integration backend.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capabilities {
    /// Whether the host supports managed profile hooks.
    pub profile_hooks: bool,
    /// Whether PSReadLine-style line editing is available.
    pub line_editor: bool,
    /// Whether completions can be registered dynamically.
    pub completions: bool,
    /// Whether support is experimental (opt-in only).
    #[serde(default)]
    pub experimental: bool,
}

/// Catalog row for `winzsh shell list`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellCatalogEntry {
    /// Shell id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Detected on this machine.
    pub detected: bool,
    /// Binary path when detected.
    pub path: Option<String>,
    /// Whether a managed hook/launcher is present.
    pub hooked: bool,
    /// Whether listed in `[shells].enabled` (PowerShell is always treated as enabled).
    pub enabled: bool,
    /// Maturity (`primary`, `stable`, `experimental`).
    pub status: String,
    /// Capability flags.
    pub capabilities: Capabilities,
}

/// Build a catalog row from a live host.
pub fn catalog_entry(
    host: &dyn ShellHost,
    detected: bool,
    path: Option<String>,
    enabled: bool,
) -> Result<ShellCatalogEntry> {
    let caps = host.capabilities();
    let status = if host.id() == ShellId::PowerShell {
        "primary".into()
    } else if caps.experimental {
        "experimental".into()
    } else {
        "stable".into()
    };
    Ok(ShellCatalogEntry {
        id: host.id().as_str().into(),
        name: host.name().into(),
        detected,
        path,
        hooked: host.hook_installed()?,
        enabled,
        status,
        capabilities: caps,
    })
}

/// Integration surface implemented per shell (PowerShell, CMD, Nushell, Bash).
pub trait ShellHost {
    /// Human-readable shell name.
    fn name(&self) -> &'static str;

    /// Stable id.
    fn id(&self) -> ShellId;

    /// Reported capabilities.
    fn capabilities(&self) -> Capabilities;

    /// Directory where the host expects modules/scripts.
    fn module_dir(&self) -> Result<PathBuf>;

    /// Path to the managed profile / config file (when applicable).
    fn profile_path(&self) -> Result<PathBuf>;

    /// Install the managed profile hook / launcher.
    fn install_hook(&self) -> Result<()>;

    /// Remove the managed profile hook / launcher bits owned by WinZSH.
    fn remove_hook(&self) -> Result<()>;

    /// Whether the managed hook is currently present.
    fn hook_installed(&self) -> Result<bool>;
}

/// Shared marker helpers for text profile files.
pub mod markers {
    /// Begin marker for non-PowerShell profile snippets.
    pub const BEGIN: &str = "# >>> winzsh >>>";
    /// End marker.
    pub const END: &str = "# <<< winzsh <<<";

    /// Insert or replace a managed block in `existing`.
    pub fn upsert(existing: &str, block: &str) -> String {
        if let Some(start) = existing.find(BEGIN) {
            if let Some(end_rel) = existing[start..].find(END) {
                let end = start + end_rel + END.len();
                let mut out = String::new();
                out.push_str(&existing[..start]);
                out.push_str(block.trim_end());
                out.push('\n');
                let rest = existing[end..].trim_start_matches(['\r', '\n']);
                if !rest.is_empty() {
                    out.push_str(rest);
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                return out;
            }
        }
        let mut out = existing.trim_end().to_string();
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(block.trim_end());
        out.push('\n');
        out
    }

    /// Remove a managed block.
    pub fn remove(existing: &str) -> String {
        if let Some(start) = existing.find(BEGIN) {
            if let Some(end_rel) = existing[start..].find(END) {
                let end = start + end_rel + END.len();
                let mut out = String::new();
                out.push_str(existing[..start].trim_end());
                let rest = existing[end..].trim_start_matches(['\r', '\n']);
                if !out.is_empty() && !rest.is_empty() {
                    out.push('\n');
                }
                out.push_str(rest);
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                return out;
            }
        }
        existing.to_string()
    }

    /// Whether markers are present.
    pub fn present(existing: &str) -> bool {
        existing.contains(BEGIN) && existing.contains(END)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_id_parse() {
        assert_eq!(ShellId::parse("pwsh"), Some(ShellId::PowerShell));
        assert_eq!(ShellId::parse("CMD"), Some(ShellId::Cmd));
        assert_eq!(ShellId::parse("nushell"), Some(ShellId::Nu));
        assert_eq!(ShellId::parse("git-bash"), Some(ShellId::Bash));
        assert_eq!(ShellId::parse("fish"), None);
    }

    #[test]
    fn markers_upsert_and_remove() {
        let block = format!(
            "{}\nhello\n{}\n",
            markers::BEGIN,
            markers::END
        );
        let merged = markers::upsert("prefix\n", &block);
        assert!(markers::present(&merged));
        assert!(merged.contains("prefix"));
        let again = markers::upsert(&merged, &format!("{}\nworld\n{}\n", markers::BEGIN, markers::END));
        assert!(again.contains("world"));
        assert!(!again.contains("hello"));
        let cleaned = markers::remove(&again);
        assert!(!markers::present(&cleaned));
        assert!(cleaned.contains("prefix"));
    }
}
