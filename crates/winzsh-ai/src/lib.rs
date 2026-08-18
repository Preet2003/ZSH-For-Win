//! AI helpers: explain, ask, safety check, alias suggest.
//!
//! **Local only** — offline heuristics, no network, no API keys.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use winzsh_error::{Result, message};

/// Compile-time phase marker.
pub const PHASE: &str = "phase-6";

/// Backend for explain/ask/alias (local heuristics only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    /// Offline heuristics (no network).
    #[default]
    Local,
}

/// Runtime AI settings (from config).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiSettings {
    /// Master enable (`features.ai`).
    pub enabled: bool,
    /// Backend provider (always local).
    pub provider: AiProvider,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: AiProvider::Local,
        }
    }
}

/// Safety severity for a scanned command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafetyLevel {
    /// No concerning patterns found.
    Ok,
    /// Potentially destructive — review carefully.
    Warn,
    /// High risk — almost certainly dangerous.
    Danger,
}

/// One safety finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SafetyFinding {
    /// Severity.
    pub level: SafetyLevel,
    /// Stable code.
    pub code: String,
    /// Human message.
    pub message: String,
    /// Optional safer alternative.
    pub safer: Option<String>,
}

/// Result of `ai check`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SafetyReport {
    /// Input command.
    pub command: String,
    /// Highest severity among findings (or Ok).
    pub level: SafetyLevel,
    /// Findings (may be empty when Ok).
    pub findings: Vec<SafetyFinding>,
}

/// Result of explain / ask / alias.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiTextResult {
    /// Provider that produced the answer.
    pub provider: String,
    /// Main answer text.
    pub text: String,
    /// Optional notes (fallback, offline, etc.).
    pub notes: Vec<String>,
}

/// Explain a shell command (opt-in).
pub fn explain(settings: &AiSettings, command: &str) -> Result<AiTextResult> {
    require_enabled(settings)?;
    let command = command.trim();
    if command.is_empty() {
        return Err(message("command must not be empty"));
    }
    Ok(explain_local(command))
}

/// Convert English intent to a PowerShell command (opt-in).
pub fn ask(settings: &AiSettings, prompt: &str) -> Result<AiTextResult> {
    require_enabled(settings)?;
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(message("prompt must not be empty"));
    }
    Ok(ask_local(prompt))
}

/// Suggest an alias name + expansion from a description (opt-in).
pub fn suggest_alias(settings: &AiSettings, description: &str) -> Result<AiTextResult> {
    require_enabled(settings)?;
    let description = description.trim();
    if description.is_empty() {
        return Err(message("description must not be empty"));
    }
    Ok(alias_local(description))
}

/// Scan a command for dangerous patterns (works even when AI is disabled).
pub fn check_safety(command: &str) -> SafetyReport {
    let command = command.trim();
    let mut findings = Vec::new();
    let lower = command.to_ascii_lowercase();

    push_if(
        &mut findings,
        lower.contains("remove-item") && (lower.contains("-recurse") || lower.contains("-r "))
            && (lower.contains("-force") || lower.contains("-f")),
        SafetyLevel::Danger,
        "ps.remove_force_recurse",
        "Recursive force delete — data may be unrecoverable",
        Some("Confirm the path; prefer Trash/Recycle when possible".into()),
    );
    push_if(
        &mut findings,
        lower.contains("rm -rf") || lower.contains("rm -fr"),
        SafetyLevel::Danger,
        "unix.rm_rf",
        "Unix-style recursive force delete",
        Some("On PowerShell prefer: Remove-Item -LiteralPath <path> -Recurse (without -Force until sure)".into()),
    );
    push_if(
        &mut findings,
        lower.contains("git push") && (lower.contains("--force") || lower.contains(" -f")),
        SafetyLevel::Danger,
        "git.force_push",
        "Force push can rewrite remote history",
        Some("Prefer git push --force-with-lease".into()),
    );
    push_if(
        &mut findings,
        lower.contains("git reset --hard"),
        SafetyLevel::Warn,
        "git.reset_hard",
        "Hard reset discards uncommitted work",
        Some("git stash before reset, or use git reset --soft".into()),
    );
    push_if(
        &mut findings,
        lower.contains("format-volume") || lower.contains("clear-disk"),
        SafetyLevel::Danger,
        "disk.destroy",
        "Disk formatting / wipe command",
        None,
    );
    push_if(
        &mut findings,
        lower.contains("invoke-expression") || lower.contains("iex ") || lower.contains("| iex"),
        SafetyLevel::Warn,
        "ps.iex",
        "Dynamic code execution (Invoke-Expression) — review the source",
        Some("Prefer calling scripts/files directly".into()),
    );
    push_if(
        &mut findings,
        lower.contains("curl ") && lower.contains("|") && (lower.contains("bash") || lower.contains("iex")),
        SafetyLevel::Danger,
        "pipe_to_shell",
        "Downloading and executing remote code",
        Some("Download first, inspect, then run".into()),
    );
    push_if(
        &mut findings,
        lower.contains(":\\windows") && (lower.contains("remove-item") || lower.contains("rm ")),
        SafetyLevel::Danger,
        "path.windows_system",
        "Touches Windows system paths",
        None,
    );

    let level = findings
        .iter()
        .map(|f| f.level)
        .max_by_key(|l| match l {
            SafetyLevel::Ok => 0,
            SafetyLevel::Warn => 1,
            SafetyLevel::Danger => 2,
        })
        .unwrap_or(SafetyLevel::Ok);

    SafetyReport {
        command: command.to_string(),
        level,
        findings,
    }
}

fn require_enabled(settings: &AiSettings) -> Result<()> {
    if settings.enabled {
        Ok(())
    } else {
        Err(message(
            "AI is disabled. Run `winzsh ai enable` (sets features.ai=true), then retry",
        ))
    }
}

fn push_if(
    out: &mut Vec<SafetyFinding>,
    cond: bool,
    level: SafetyLevel,
    code: &str,
    message: &str,
    safer: Option<String>,
) {
    if cond {
        out.push(SafetyFinding {
            level,
            code: code.into(),
            message: message.into(),
            safer,
        });
    }
}

fn explain_local(command: &str) -> AiTextResult {
    let lower = command.to_ascii_lowercase();
    let mut parts = Vec::new();

    if lower.starts_with("git ") {
        parts.push("Git version-control command.".into());
        if lower.contains("status") {
            parts.push("Shows working tree / staging status.".into());
        } else if lower.contains("commit") {
            parts.push("Records a snapshot of staged changes.".into());
        } else if lower.contains("push") {
            parts.push("Uploads local commits to a remote.".into());
        } else if lower.contains("pull") {
            parts.push("Fetches and integrates remote changes.".into());
        } else if lower.contains("reset") {
            parts.push("Moves HEAD / staging; can discard commits or changes.".into());
        }
    } else if lower.starts_with("docker ") {
        parts.push("Docker CLI — manages containers/images/compose.".into());
    } else if lower.starts_with("kubectl ") || lower.starts_with("k ") {
        parts.push("Kubernetes CLI — talks to a cluster API.".into());
    } else if lower.contains("get-childitem")
        || lower.split_whitespace().next() == Some("ls")
        || lower.split_whitespace().next() == Some("dir")
    {
        parts.push("Lists files and directories in the current (or given) path.".into());
    } else if lower.contains("remove-item") || lower.starts_with("rm ") || lower.starts_with("del ")
    {
        parts.push("Deletes files or directories.".into());
    } else if lower.contains("set-location") || lower.starts_with("cd ") {
        parts.push("Changes the current directory.".into());
    } else if lower.starts_with("cargo ") {
        parts.push("Rust Cargo toolchain command (build/test/run/…).".into());
    } else {
        parts.push(
            "Local explain: no specific heuristic matched for this command.".into(),
        );
        parts.push(format!("Command: {command}"));
    }

    let safety = check_safety(command);
    if safety.level != SafetyLevel::Ok {
        for f in &safety.findings {
            parts.push(format!("⚠ {:?} [{}]: {}", f.level, f.code, f.message));
        }
    }

    AiTextResult {
        provider: "local".into(),
        text: parts.join("\n"),
        notes: vec!["provider=local (offline heuristics)".into()],
    }
}

fn ask_local(prompt: &str) -> AiTextResult {
    let p = prompt.to_ascii_lowercase();
    let text = if (p.contains("delete") || p.contains("remove")) && p.contains("node_modules") {
        "Remove-Item -LiteralPath .\\node_modules -Recurse -Force".into()
    } else if p.contains("list") && (p.contains("file") || p.contains("dir") || p.contains("folder"))
    {
        "Get-ChildItem -Force".into()
    } else if p.contains("git status") || (p.contains("git") && p.contains("status")) {
        "git status -sb".into()
    } else if p.contains("docker") && (p.contains("ps") || p.contains("container")) {
        "docker ps".into()
    } else if (p.contains("build") && p.contains("rust"))
        || (p.contains("cargo") && p.contains("build"))
    {
        "cargo build".into()
    } else if p.contains("test") && (p.contains("cargo") || p.contains("rust")) {
        "cargo test".into()
    } else if p.contains("current directory") || p.contains("where am i") || p.contains("pwd") {
        "Get-Location".into()
    } else if p.contains("clear") && p.contains("screen") {
        "Clear-Host".into()
    } else {
        format!(
            "# No local mapping for: {prompt}\n# WinZSH AI is local-only (offline heuristics)."
        )
    };

    let mut notes = vec!["provider=local (offline heuristics)".into()];
    if !text.starts_with('#') {
        let safety = check_safety(&text);
        if safety.level != SafetyLevel::Ok {
            notes.push(format!(
                "safety: {:?} — review with `winzsh ai check \"{text}\"`",
                safety.level
            ));
        }
        notes.push("Review before running. This is a suggestion, not an auto-execute.".into());
    }

    AiTextResult {
        provider: "local".into(),
        text,
        notes,
    }
}

fn alias_local(description: &str) -> AiTextResult {
    let lower = description.to_ascii_lowercase();
    let (name, value) = if lower.contains("git status") {
        ("gst", "git status -sb")
    } else if lower.contains("git push") {
        ("gpush", "git push")
    } else if lower.contains("docker ps") || (lower.contains("docker") && lower.contains("container"))
    {
        ("dps", "docker ps")
    } else if lower.contains("cargo build") {
        ("cb", "cargo build")
    } else if lower.contains("cargo test") {
        ("ct", "cargo test")
    } else if lower.contains("list") && lower.contains("file") {
        ("ll", "Get-ChildItem -Force")
    } else {
        ("myalias", "Write-Host 'replace-me'")
    };

    AiTextResult {
        provider: "local".into(),
        text: format!("NAME={name}\nVALUE={value}"),
        notes: vec![
            "provider=local (offline heuristics)".into(),
            format!("Install with: winzsh alias set {name} \"{value}\""),
            format!("Or session-only: salias {name} {value}"),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_flags_force_push() {
        let r = check_safety("git push --force origin main");
        assert_eq!(r.level, SafetyLevel::Danger);
        assert!(r.findings.iter().any(|f| f.code == "git.force_push"));
    }

    #[test]
    fn ask_local_node_modules() {
        let settings = AiSettings {
            enabled: true,
            ..AiSettings::default()
        };
        let r = ask(&settings, "delete node_modules recursively").expect("ask");
        assert!(r.text.to_ascii_lowercase().contains("node_modules"));
        assert_eq!(r.provider, "local");
    }

    #[test]
    fn explain_requires_enable() {
        let settings = AiSettings::default();
        assert!(explain(&settings, "git status").is_err());
    }

    #[test]
    fn explain_git_local() {
        let settings = AiSettings {
            enabled: true,
            ..AiSettings::default()
        };
        let r = explain(&settings, "git status").expect("ok");
        assert!(r.text.to_ascii_lowercase().contains("git"));
    }
}
