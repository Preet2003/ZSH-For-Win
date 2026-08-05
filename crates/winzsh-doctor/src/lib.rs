//! Diagnostics and remediation hints consumed by CLI and installer verify.

#![forbid(unsafe_code)]

use serde::Serialize;
use winzsh_config::{self as config};
use winzsh_core::WinzshPaths;
use winzsh_detect::detect_environment;
use winzsh_powershell::{PowerShellHost, runtime_module_exists};
use winzsh_shell_host::ShellHost;

/// Severity of a diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational.
    Info,
    /// Warning; shell may still work.
    Warning,
    /// Error; user action required.
    Error,
}

/// One structured diagnostic.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// Severity.
    pub severity: Severity,
    /// Stable machine code.
    pub code: String,
    /// Human message.
    pub message: String,
    /// Optional remediation hint.
    pub hint: Option<String>,
}

impl Diagnostic {
    fn error(code: &str, message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: code.to_string(),
            message: message.into(),
            hint: Some(hint.into()),
        }
    }

    fn warning(code: &str, message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.to_string(),
            message: message.into(),
            hint: Some(hint.into()),
        }
    }

    fn info(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            code: code.to_string(),
            message: message.into(),
            hint: None,
        }
    }
}

/// Full doctor report.
#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    /// Findings.
    pub diagnostics: Vec<Diagnostic>,
    /// True when no error-severity findings exist.
    pub ok: bool,
}

impl DoctorReport {
    /// Highest severity is Error?
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// Run diagnostics against the current environment and install layout.
pub fn run(paths: &WinzshPaths) -> DoctorReport {
    let mut diagnostics = Vec::new();

    let env = match detect_environment() {
        Ok(env) => env,
        Err(e) => {
            diagnostics.push(Diagnostic::error(
                "detect.failed",
                e.to_string(),
                "Ensure PowerShell 7 (recommended) or Windows PowerShell is installed and on PATH",
            ));
            return finish(diagnostics);
        }
    };

    if let Some(pwsh) = &env.pwsh {
        diagnostics.push(Diagnostic::info(
            "detect.pwsh",
            format!("PowerShell 7 found at {}", pwsh.display()),
        ));
    } else if env.has_windows_powershell() {
        diagnostics.push(Diagnostic::warning(
            "detect.pwsh_missing",
            "PowerShell 7 (pwsh) was not found; using Windows PowerShell",
            "Install PowerShell 7 from https://aka.ms/powershell for the best WinZSH experience",
        ));
        if let Some(powershell) = &env.windows_powershell {
            diagnostics.push(Diagnostic::info(
                "detect.windows_powershell",
                format!("Windows PowerShell found at {}", powershell.display()),
            ));
        }
    } else {
        diagnostics.push(Diagnostic::error(
            "detect.powershell_missing",
            "No PowerShell host (pwsh or powershell) was found on PATH",
            "Install PowerShell 7 from https://aka.ms/powershell and re-open the terminal",
        ));
    }

    if env.git.is_some() {
        diagnostics.push(Diagnostic::info("detect.git", "Git found on PATH"));
    } else {
        diagnostics.push(Diagnostic::warning(
            "detect.git_missing",
            "Git was not found on PATH",
            "Install Git for Windows for git-aware prompt features",
        ));
    }

    if env.fzf.is_some() {
        diagnostics.push(Diagnostic::info(
            "detect.fzf",
            "fzf found — Ctrl+R fuzzy history enabled when smart.fzf=true",
        ));
    } else {
        diagnostics.push(Diagnostic::info(
            "detect.fzf_missing",
            "fzf not found (optional); install for Ctrl+R fuzzy history",
        ));
    }

    if env.zoxide.is_some() {
        diagnostics.push(Diagnostic::info(
            "detect.zoxide",
            "zoxide found — will initialize when smart.zoxide=true",
        ));
    } else {
        diagnostics.push(Diagnostic::info(
            "detect.zoxide_missing",
            "zoxide not found (optional); install for fast directory jumping (`z`)",
        ));
    }

    if !paths.root.exists() {
        diagnostics.push(Diagnostic::error(
            "install.missing_home",
            format!("WinZSH home missing at {}", paths.root.display()),
            "Run `winzsh install`",
        ));
        return finish(diagnostics);
    }

    if !paths.is_installed() {
        diagnostics.push(Diagnostic::error(
            "install.missing_state",
            "state.json missing — WinZSH does not look installed",
            "Run `winzsh install`",
        ));
    } else {
        diagnostics.push(Diagnostic::info(
            "install.state",
            format!("state.json present at {}", paths.state_file().display()),
        ));
    }

    match config::load(paths) {
        Ok(cfg) => {
            if let Err(e) = config::validate(&cfg) {
                diagnostics.push(Diagnostic::error(
                    "config.invalid",
                    e.to_string(),
                    "Fix ~/.winzsh/config.toml or re-run `winzsh install`",
                ));
            } else if let Err(e) = winzsh_theme::validate_id(&cfg.theme) {
                diagnostics.push(Diagnostic::error(
                    "theme.unknown",
                    e.to_string(),
                    "Run `winzsh theme list` and `winzsh theme set <id>`",
                ));
            } else {
                diagnostics.push(Diagnostic::info(
                    "config.ok",
                    format!("config OK (theme={})", cfg.theme),
                ));
            }
        }
        Err(e) => diagnostics.push(Diagnostic::error(
            "config.missing",
            e.to_string(),
            "Run `winzsh install` to create a default config",
        )),
    }

    if paths.runtime_module().is_file() {
        match std::fs::read_to_string(paths.runtime_module()) {
            Ok(module) => {
                if module.contains("function prompt") {
                    diagnostics.push(Diagnostic::info(
                        "prompt.present",
                        "runtime module includes prompt",
                    ));
                } else {
                    diagnostics.push(Diagnostic::warning(
                        "prompt.missing",
                        "runtime module has no prompt function",
                        "Run `winzsh reload` or `winzsh install --force`",
                    ));
                }
                if module.contains("AcceptSuggestion") {
                    diagnostics.push(Diagnostic::info(
                        "suggest.accept",
                        "runtime module wires RightArrow → AcceptSuggestion",
                    ));
                } else {
                    diagnostics.push(Diagnostic::warning(
                        "suggest.missing",
                        "runtime module missing autosuggest accept handler",
                        "Run `winzsh reload` to upgrade to Phase 3",
                    ));
                }
            }
            Err(e) => diagnostics.push(Diagnostic::warning(
                "prompt.unreadable",
                e.to_string(),
                "Run `winzsh reload`",
            )),
        }
    }

    if runtime_module_exists(paths) {
        diagnostics.push(Diagnostic::info(
            "runtime.module",
            format!(
                "runtime module present at {}",
                paths.runtime_module().display()
            ),
        ));
    } else {
        diagnostics.push(Diagnostic::error(
            "runtime.missing",
            "Cached runtime module is missing",
            "Run `winzsh install` or repair with a re-install",
        ));
    }

    if let Some(profile_path) = env.profile_path.clone() {
        let host = PowerShellHost::new(paths.clone(), profile_path.clone());
        match host.hook_installed() {
            Ok(true) => diagnostics.push(Diagnostic::info(
                "profile.hook",
                format!("profile hook present in {}", profile_path.display()),
            )),
            Ok(false) => diagnostics.push(Diagnostic::error(
                "profile.hook_missing",
                format!("WinZSH hook missing from {}", profile_path.display()),
                "Run `winzsh install`",
            )),
            Err(e) => diagnostics.push(Diagnostic::error(
                "profile.hook_error",
                e.to_string(),
                "Inspect the profile markers or restore a backup under ~/.winzsh/backups/profile",
            )),
        }
    }

    finish(diagnostics)
}

fn finish(diagnostics: Vec<Diagnostic>) -> DoctorReport {
    let ok = !diagnostics.iter().any(|d| d.severity == Severity::Error);
    DoctorReport { diagnostics, ok }
}
