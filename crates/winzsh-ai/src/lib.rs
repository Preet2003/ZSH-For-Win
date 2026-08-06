//! AI helpers for Phase 6: explain, ask, safety check, alias suggest.
//!
//! Default provider is **local** (offline heuristics). Optional OpenAI-compatible
//! HTTP when `provider = "openai"` and an API key is present.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use winzsh_error::{Result, message};

/// Compile-time phase marker.
pub const PHASE: &str = "phase-6";

/// Which backend answers explain/ask/alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    /// Offline heuristics (no network).
    #[default]
    Local,
    /// OpenAI-compatible Chat Completions API.
    Openai,
}

/// Runtime AI settings (from config + env).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiSettings {
    /// Master enable (`features.ai`).
    pub enabled: bool,
    /// Backend provider.
    pub provider: AiProvider,
    /// Model id for cloud provider.
    pub model: String,
    /// API base URL (OpenAI-compatible).
    pub api_base: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: AiProvider::Local,
            model: "gpt-4o-mini".into(),
            api_base: "https://api.openai.com/v1".into(),
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

/// Resolve API key from environment (never logged).
pub fn api_key_from_env() -> Option<String> {
    for name in ["WINZSH_AI_API_KEY", "OPENAI_API_KEY"] {
        if let Ok(v) = std::env::var(name) {
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// Whether cloud calls can be attempted.
pub fn cloud_ready(settings: &AiSettings) -> bool {
    settings.enabled
        && settings.provider == AiProvider::Openai
        && api_key_from_env().is_some()
}

/// Explain a shell command (opt-in).
pub fn explain(settings: &AiSettings, command: &str) -> Result<AiTextResult> {
    require_enabled(settings)?;
    let command = command.trim();
    if command.is_empty() {
        return Err(message("command must not be empty"));
    }
    if cloud_ready(settings) {
        match chat(
            settings,
            &system_prompt_explain(),
            &format!("Explain this PowerShell/Windows shell command for a developer:\n\n{command}"),
        ) {
            Ok(text) => {
                return Ok(AiTextResult {
                    provider: "openai".into(),
                    text,
                    notes: Vec::new(),
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "openai explain failed; falling back to local");
                let mut local = explain_local(command);
                local.notes.push(format!("cloud explain failed: {e}"));
                return Ok(local);
            }
        }
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
    if cloud_ready(settings) {
        match chat(
            settings,
            &system_prompt_ask(),
            &format!(
                "Convert this request into a single PowerShell 7 command. Reply with ONLY the command, no markdown:\n\n{prompt}"
            ),
        ) {
            Ok(text) => {
                let cmd = strip_code_fence(&text);
                let mut notes = Vec::new();
                let safety = check_safety(&cmd);
                if safety.level != SafetyLevel::Ok {
                    notes.push(format!(
                        "safety: {:?} — run `winzsh ai check` before executing",
                        safety.level
                    ));
                }
                return Ok(AiTextResult {
                    provider: "openai".into(),
                    text: cmd,
                    notes,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "openai ask failed; falling back to local");
                let mut local = ask_local(prompt);
                local.notes.push(format!("cloud ask failed: {e}"));
                return Ok(local);
            }
        }
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
    if cloud_ready(settings) {
        match chat(
            settings,
            "You suggest short PowerShell aliases for WinZSH. Reply with exactly two lines:\nNAME=<alias>\nVALUE=<expansion>",
            description,
        ) {
            Ok(text) => {
                return Ok(AiTextResult {
                    provider: "openai".into(),
                    text: format_alias_suggestion(&text, description),
                    notes: vec![
                        "Install with: winzsh alias set <name> \"<value>\"".into(),
                    ],
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "openai alias failed; falling back to local");
                let mut local = alias_local(description);
                local.notes.push(format!("cloud alias failed: {e}"));
                return Ok(local);
            }
        }
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
    } else if lower.contains("get-childitem") || lower.split_whitespace().next() == Some("ls")
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
            "Local explain: no specific heuristic matched. Enable OpenAI provider + API key for richer explanations."
                .into(),
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
            "# No local mapping for: {prompt}\n# Tip: set [ai] provider=\"openai\" and WINZSH_AI_API_KEY for cloud conversion"
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

fn format_alias_suggestion(raw: &str, fallback_desc: &str) -> String {
    let mut name = None;
    let mut value = None;
    for line in raw.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("NAME=") {
            name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("VALUE=") {
            value = Some(rest.trim().to_string());
        }
    }
    match (name, value) {
        (Some(n), Some(v)) => format!("NAME={n}\nVALUE={v}"),
        _ => alias_local(fallback_desc).text,
    }
}

fn strip_code_fence(text: &str) -> String {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest
            .strip_prefix("powershell")
            .or_else(|| rest.strip_prefix("pwsh"))
            .or_else(|| rest.strip_prefix("bash"))
            .unwrap_or(rest);
        let rest = rest.trim_start_matches('\n');
        if let Some(end) = rest.find("```") {
            return rest[..end].trim().to_string();
        }
    }
    t.lines()
        .next()
        .unwrap_or(t)
        .trim()
        .trim_start_matches('$')
        .to_string()
}

fn system_prompt_explain() -> String {
    "You are WinZSH AI. Explain shell commands clearly for Windows PowerShell developers. Mention risks briefly. Keep under 8 sentences.".into()
}

fn system_prompt_ask() -> String {
    "You are WinZSH AI. Output a single PowerShell 7 command only. Prefer PowerShell cmdlets over bash. Never wrap in markdown.".into()
}

fn chat(settings: &AiSettings, system: &str, user: &str) -> Result<String> {
    let key = api_key_from_env().ok_or_else(|| message("missing WINZSH_AI_API_KEY / OPENAI_API_KEY"))?;
    let url = format!(
        "{}/chat/completions",
        settings.api_base.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "model": settings.model,
        "temperature": 0.2,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
    });

    let resp = ureq::post(&url)
        .set("Authorization", &format!("Bearer {key}"))
        .set("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(45))
        .send_json(body)
        .map_err(|e| message(format!("AI HTTP error: {e}")))?;

    let status = resp.status();
    let value: serde_json::Value = resp
        .into_json()
        .map_err(|e| message(format!("AI response parse error: {e}")))?;

    if !(200..300).contains(&status) {
        let err = value
            .pointer("/error/message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown API error");
        return Err(message(format!("AI API {status}: {err}")));
    }

    value
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| message("AI API returned empty content"))
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

    #[test]
    fn strip_fence() {
        assert_eq!(
            strip_code_fence("```powershell\nGet-ChildItem\n```"),
            "Get-ChildItem"
        );
    }
}
