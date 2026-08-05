//! PowerShell 7 profile markers, module path registration, and host integration.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use tracing::{debug, info};
use winzsh_core::WinzshPaths;
use winzsh_error::{Result, profile};
use winzsh_fs::{atomic_write, backup_file, ensure_dir, read_string};
use winzsh_shell_host::{Capabilities, ShellHost};

/// Marker comments owned exclusively by the installer / powershell crate.
pub const PROFILE_BEGIN: &str = "# >>> winzsh >>>";
/// End marker for the managed profile block.
pub const PROFILE_END: &str = "# <<< winzsh <<<";

/// PowerShell 7 host integration.
#[derive(Debug, Clone)]
pub struct PowerShellHost {
    /// WinZSH paths (runtime module location).
    pub paths: WinzshPaths,
    /// Profile file to manage.
    pub profile_path: PathBuf,
}

impl PowerShellHost {
    /// Construct a host for the given paths and profile.
    pub fn new(paths: WinzshPaths, profile_path: PathBuf) -> Self {
        Self {
            paths,
            profile_path,
        }
    }

    /// Render the managed hook block that loads the cached runtime module.
    pub fn render_hook(&self) -> String {
        let module = self.paths.runtime_module();
        let module_display = module.display();
        format!(
            r#"{PROFILE_BEGIN}
# Managed by WinZSH — do not edit this block by hand.
$__winzshModule = '{module_display}'
if (Test-Path -LiteralPath $__winzshModule) {{
    Import-Module -Name $__winzshModule -Force -ErrorAction Stop
}} else {{
    Write-Warning "WinZSH runtime module missing at $__winzshModule. Run 'winzsh doctor'."
}}
Remove-Variable __winzshModule -ErrorAction SilentlyContinue
{PROFILE_END}
"#
        )
    }

    /// Backup the current profile if it exists.
    pub fn backup_profile(&self) -> Result<Option<PathBuf>> {
        if !self.profile_path.is_file() {
            return Ok(None);
        }
        let dest = backup_file(&self.profile_path, &self.paths.profile_backups(), "profile")?;
        info!(backup = %dest.display(), "backed up PowerShell profile");
        Ok(Some(dest))
    }
}

impl ShellHost for PowerShellHost {
    fn name(&self) -> &'static str {
        "powershell"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            profile_hooks: true,
            line_editor: true,
            completions: true,
        }
    }

    fn module_dir(&self) -> Result<PathBuf> {
        Ok(self.paths.runtime_cache())
    }

    fn profile_path(&self) -> Result<PathBuf> {
        Ok(self.profile_path.clone())
    }

    fn install_hook(&self) -> Result<()> {
        if let Some(parent) = self.profile_path.parent() {
            ensure_dir(parent)?;
        }
        let hook = self.render_hook();
        let new_contents = if self.profile_path.is_file() {
            let existing = read_string(&self.profile_path)?;
            upsert_hook_block(&existing, &hook)?
        } else {
            format!("{hook}\n")
        };
        atomic_write(&self.profile_path, new_contents)?;
        info!(profile = %self.profile_path.display(), "installed WinZSH profile hook");
        Ok(())
    }

    fn remove_hook(&self) -> Result<()> {
        if !self.profile_path.is_file() {
            debug!("profile missing; nothing to remove");
            return Ok(());
        }
        let existing = read_string(&self.profile_path)?;
        let updated = remove_hook_block(&existing)?;
        atomic_write(&self.profile_path, updated)?;
        info!(profile = %self.profile_path.display(), "removed WinZSH profile hook");
        Ok(())
    }

    fn hook_installed(&self) -> Result<bool> {
        if !self.profile_path.is_file() {
            return Ok(false);
        }
        let existing = read_string(&self.profile_path)?;
        Ok(existing.contains(PROFILE_BEGIN) && existing.contains(PROFILE_END))
    }
}

/// Insert or replace the managed hook block inside a profile script.
pub fn upsert_hook_block(existing: &str, hook: &str) -> Result<String> {
    if existing.contains(PROFILE_BEGIN) || existing.contains(PROFILE_END) {
        let without = remove_hook_block(existing)?;
        if without.trim().is_empty() {
            Ok(format!("{}\n", hook.trim_end()))
        } else {
            Ok(format!("{}\n{}\n", without.trim_end(), hook.trim_end()))
        }
    } else if existing.trim().is_empty() {
        Ok(format!("{}\n", hook.trim_end()))
    } else {
        Ok(format!("{}\n{}\n", existing.trim_end(), hook.trim_end()))
    }
}

/// Remove the managed hook block from a profile script.
pub fn remove_hook_block(existing: &str) -> Result<String> {
    let begin = existing.find(PROFILE_BEGIN);
    let end = existing.find(PROFILE_END);
    match (begin, end) {
        (None, None) => Ok(existing.to_string()),
        (Some(b), Some(e)) if e >= b => {
            let after = e + PROFILE_END.len();
            let mut out = String::new();
            out.push_str(&existing[..b]);
            let rest = existing[after..].trim_start_matches(['\r', '\n']);
            out.push_str(rest);
            Ok(out)
        }
        _ => Err(profile(
            "malformed WinZSH profile markers; fix or restore a profile backup",
        )),
    }
}

/// Whether `path` looks like a generated runtime module file.
pub fn runtime_module_exists(paths: &WinzshPaths) -> bool {
    Path::new(&paths.runtime_module()).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_and_remove_roundtrip() {
        let paths = WinzshPaths::from_root(PathBuf::from("C:/tmp/winzsh-test"));
        let host = PowerShellHost::new(paths, PathBuf::from("C:/tmp/profile.ps1"));
        let hook = host.render_hook();
        let with = upsert_hook_block("Write-Host hi\n", &hook).expect("upsert");
        assert!(with.contains(PROFILE_BEGIN));
        let gone = remove_hook_block(&with).expect("remove");
        assert!(!gone.contains(PROFILE_BEGIN));
        assert!(gone.contains("Write-Host hi"));
    }
}
