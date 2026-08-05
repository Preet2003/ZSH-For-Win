//! Tool and environment detection for lazy feature enablement.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use winzsh_core::PROFILE_ENV;
use winzsh_error::{Result, detect};

/// Snapshot of detected host tools.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DetectionReport {
    /// Path to PowerShell 7 (`pwsh`) if found.
    pub pwsh: Option<PathBuf>,
    /// Path to Windows PowerShell 5.1 (`powershell.exe`) if found.
    pub windows_powershell: Option<PathBuf>,
    /// Path to `git` if found.
    pub git: Option<PathBuf>,
    /// Path to Windows Terminal (`wt.exe`) if found.
    pub windows_terminal: Option<PathBuf>,
    /// Path to `fzf` if found.
    pub fzf: Option<PathBuf>,
    /// Path to `zoxide` if found.
    pub zoxide: Option<PathBuf>,
    /// Resolved PowerShell user profile path.
    pub profile_path: Option<PathBuf>,
    /// Names of additional detected commands on PATH.
    pub commands: Vec<String>,
}

impl DetectionReport {
    /// Whether PowerShell 7 appears available.
    pub fn has_pwsh(&self) -> bool {
        self.pwsh.is_some()
    }

    /// Whether Windows PowerShell 5.1 appears available.
    pub fn has_windows_powershell(&self) -> bool {
        self.windows_powershell.is_some()
    }

    /// Whether any supported PowerShell host is available.
    pub fn has_powershell_host(&self) -> bool {
        self.has_pwsh() || self.has_windows_powershell()
    }

    /// Preferred shell binary: PowerShell 7 when present, otherwise Windows PowerShell.
    pub fn preferred_shell(&self) -> Option<&Path> {
        self.pwsh.as_deref().or(self.windows_powershell.as_deref())
    }
}

/// Run environment detection.
pub fn detect_environment() -> Result<DetectionReport> {
    let pwsh = find_pwsh();
    let windows_powershell = find_windows_powershell();
    let git = find_on_path("git");
    let windows_terminal = find_on_path("wt");
    let fzf = find_on_path("fzf").or_else(|| find_winget_tool("fzf"));
    let zoxide = find_on_path("zoxide").or_else(|| find_winget_tool("zoxide"));
    let profile_path = resolve_profile_path(pwsh.as_deref().or(windows_powershell.as_deref()))?;

    let mut commands = Vec::new();
    if pwsh.is_some() {
        commands.push("pwsh".to_string());
    }
    if windows_powershell.is_some() {
        commands.push("powershell".to_string());
    }
    if git.is_some() {
        commands.push("git".to_string());
    }
    if windows_terminal.is_some() {
        commands.push("wt".to_string());
    }
    if fzf.is_some() {
        commands.push("fzf".to_string());
    }
    if zoxide.is_some() {
        commands.push("zoxide".to_string());
    }
    // Phase 4 developer CLIs (completion packs)
    for name in [
        "docker", "kubectl", "npm", "pnpm", "yarn", "terraform", "ssh", "aws", "az", "cargo",
    ] {
        if find_on_path(name)
            .or_else(|| find_winget_tool(name))
            .is_some()
        {
            commands.push(name.to_string());
        }
    }

    Ok(DetectionReport {
        pwsh,
        windows_powershell,
        git,
        windows_terminal,
        fzf,
        zoxide,
        profile_path: Some(profile_path),
        commands,
    })
}

/// Resolve the PowerShell profile path (env override, shell query, or conventional path).
pub fn resolve_profile_path(shell: Option<&Path>) -> Result<PathBuf> {
    if let Ok(override_path) = std::env::var(PROFILE_ENV) {
        if !override_path.trim().is_empty() {
            return Ok(PathBuf::from(override_path));
        }
    }

    if let Some(shell) = shell {
        if let Some(path) = query_shell_profile(shell) {
            return Ok(path);
        }
        return conventional_profile_path(is_windows_powershell_binary(shell));
    }

    conventional_profile_path(false)
}

fn query_shell_profile(shell: &Path) -> Option<PathBuf> {
    let output = Command::new(shell)
        .args([
            "-NoProfile",
            "-NoLogo",
            "-Command",
            "$PROFILE.CurrentUserAllHosts",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    }
}

fn conventional_profile_path(windows_powershell: bool) -> Result<PathBuf> {
    let docs =
        dirs::document_dir().ok_or_else(|| detect("could not resolve Documents directory"))?;
    let folder = if windows_powershell {
        "WindowsPowerShell"
    } else {
        "PowerShell"
    };
    Ok(docs.join(folder).join("Microsoft.PowerShell_profile.ps1"))
}

fn is_windows_powershell_binary(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("powershell"))
}

/// Locate PowerShell 7 (`pwsh`), checking PATH then well-known install locations.
pub fn find_pwsh() -> Option<PathBuf> {
    find_on_path("pwsh").or_else(find_pwsh_well_known)
}

/// Locate Windows PowerShell 5.1 (`powershell.exe`).
pub fn find_windows_powershell() -> Option<PathBuf> {
    find_on_path("powershell").or_else(find_windows_powershell_well_known)
}

fn find_pwsh_well_known() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from)?;
    for rel in ["PowerShell\\7\\pwsh.exe", "PowerShell\\7-preview\\pwsh.exe"] {
        let candidate = program_files.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_windows_powershell_well_known() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let system_root = std::env::var_os("SystemRoot").map(PathBuf::from)?;
    let candidate = system_root
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    candidate.is_file().then_some(candidate)
}

/// Search WinGet package/link folders for a tool (handles stale session PATH).
pub fn find_winget_tool(name: &str) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let links = local
        .join("Microsoft")
        .join("WinGet")
        .join("Links")
        .join(format!("{name}.exe"));
    if links.is_file() {
        return Some(links);
    }
    let packages = local.join("Microsoft").join("WinGet").join("Packages");
    if !packages.is_dir() {
        return None;
    }
    let mut stack = vec![packages];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case(&format!("{name}.exe")))
            {
                return Some(path);
            }
        }
    }
    None
}

/// Find an executable on PATH (Windows-aware).
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var_os("PATHEXT")
            .map(|v| {
                v.to_string_lossy()
                    .split(';')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_else(|| vec![".EXE".into(), ".CMD".into(), ".BAT".into()])
    } else {
        vec![String::new()]
    };

    for dir in std::env::split_paths(&path_var) {
        if !cfg!(windows) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
            continue;
        }
        // exact name
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct);
        }
        for ext in &exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
            // case variants on Windows are usually handled by FS; try lowercase ext too
            let candidate_lower = dir.join(format!("{name}{}", ext.to_lowercase()));
            if candidate_lower.is_file() {
                return Some(candidate_lower);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventional_profile_not_empty() {
        // May fail in exotic CI without a home; skip soft.
        if dirs::document_dir().is_some() {
            let path = conventional_profile_path(false).expect("profile");
            assert!(path.to_string_lossy().contains("PowerShell"));
            let win_path = conventional_profile_path(true).expect("win profile");
            assert!(win_path.to_string_lossy().contains("WindowsPowerShell"));
        }
    }

    #[test]
    fn preferred_shell_prefers_pwsh() {
        let report = DetectionReport {
            pwsh: Some(PathBuf::from("C:/Program Files/PowerShell/7/pwsh.exe")),
            windows_powershell: Some(PathBuf::from(
                "C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe",
            )),
            ..DetectionReport::default()
        };
        assert!(report.has_powershell_host());
        assert_eq!(
            report
                .preferred_shell()
                .map(|p| p.to_string_lossy().into_owned()),
            Some("C:/Program Files/PowerShell/7/pwsh.exe".into())
        );
    }

    #[test]
    fn is_windows_powershell_binary_detects_name() {
        assert!(is_windows_powershell_binary(Path::new(
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
        )));
        assert!(!is_windows_powershell_binary(Path::new(
            r"C:\Program Files\PowerShell\7\pwsh.exe"
        )));
    }
}
