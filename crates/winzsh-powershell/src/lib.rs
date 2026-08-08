//! PowerShell 7 profile markers, module path registration, and host integration.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use tracing::{debug, info};
use winzsh_core::{SHELL_ENV, WinzshPaths};
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
    ///
    /// Plain PowerShell only gets the `zsh-for-win` launcher. The full WinZSH
    /// runtime loads when `{SHELL_ENV}=1` (set by that launcher for a nested session).
    ///
    /// A shared `locks/shell.active` file makes activation global: other stock
    /// terminals auto-join at profile load or on idle (never from inside `prompt`,
    /// which breaks ConPTY nesting). `exit` in any nested session clears the lock.
    pub fn render_hook(&self) -> String {
        let module = self.paths.runtime_module();
        let module_display = module.display();
        let active_lock = self.paths.shell_active_lock();
        let active_lock_display = active_lock.display();
        format!(
            r#"{PROFILE_BEGIN}
# Managed by WinZSH — do not edit this block by hand.
# Stock PowerShell stays normal. Run `zsh-for-win` to enter WinZSH everywhere;
# `exit` in any WinZSH session returns every terminal to stock.
$global:__WinZshActiveLock = '{active_lock_display}'
function global:zsh-for-win {{
    [CmdletBinding()]
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$PwshArgs
    )
    if ($env:{SHELL_ENV} -eq '1') {{
        Write-Host 'Already inside a WinZSH session.' -ForegroundColor Yellow
        return
    }}
    $lockDir = Split-Path -Parent $global:__WinZshActiveLock
    if (-not (Test-Path -LiteralPath $lockDir)) {{
        New-Item -ItemType Directory -Path $lockDir -Force | Out-Null
    }}
    New-Item -ItemType File -Path $global:__WinZshActiveLock -Force | Out-Null
    $prev = [System.Environment]::GetEnvironmentVariable('{SHELL_ENV}', 'Process')
    [System.Environment]::SetEnvironmentVariable('{SHELL_ENV}', '1', 'Process')
    try {{
        $pwshCmd = Get-Command pwsh -ErrorAction SilentlyContinue
        if ($pwshCmd) {{
            & $pwshCmd.Source -NoLogo @PwshArgs
        }} else {{
            & powershell -NoLogo @PwshArgs
        }}
    }} finally {{
        Remove-Item -LiteralPath $global:__WinZshActiveLock -Force -ErrorAction SilentlyContinue
        if ($null -eq $prev -or $prev -eq '') {{
            Remove-Item -Path Env:{SHELL_ENV} -ErrorAction SilentlyContinue
        }} else {{
            [System.Environment]::SetEnvironmentVariable('{SHELL_ENV}', $prev, 'Process')
        }}
    }}
}}
function global:Enter-WinZshSessionIfActive {{
    if ($env:{SHELL_ENV} -eq '1') {{ return }}
    if ($global:__WinZshEntering) {{ return }}
    if (-not (Test-Path -LiteralPath $global:__WinZshActiveLock)) {{ return }}
    $global:__WinZshEntering = $true
    try {{
        Write-Host 'WinZSH is active — joining session. Type exit to deactivate everywhere.' -ForegroundColor Cyan
        zsh-for-win
    }} finally {{
        $global:__WinZshEntering = $false
    }}
}}
if ($env:{SHELL_ENV} -eq '1') {{
    $__winzshModule = '{module_display}'
    if (Test-Path -LiteralPath $__winzshModule) {{
        Import-Module -Name $__winzshModule -Force -ErrorAction Stop
        Write-Host ("WinZSH · " + (Get-WinZshInfo).Theme + " · type exit to deactivate everywhere") -ForegroundColor Magenta
    }} else {{
        Write-Warning "WinZSH runtime module missing at $__winzshModule. Run 'winzsh doctor'."
    }}
    Remove-Variable __winzshModule -ErrorAction SilentlyContinue
}} else {{
    # Stock session: keep PowerShell plain (PSReadLine history ghosts are native, not WinZSH).
    if (Get-Module -ListAvailable -Name PSReadLine) {{
        try {{
            Set-PSReadLineOption -PredictionSource None -ErrorAction SilentlyContinue
        }} catch {{ }}
    }}
    # Join at profile load (new tabs) or on idle (already-open tabs) — never inside prompt.
    if (Test-Path -LiteralPath $global:__WinZshActiveLock) {{
        Enter-WinZshSessionIfActive
    }} else {{
        Register-EngineEvent -SourceIdentifier PowerShell.OnIdle -Action {{
            if ((Test-Path -LiteralPath $global:__WinZshActiveLock) -and ($env:{SHELL_ENV} -ne '1')) {{
                Enter-WinZshSessionIfActive
            }}
        }} | Out-Null
    }}
}}
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
        assert!(with.contains("function global:zsh-for-win"));
        assert!(with.contains("WINZSH_SHELL"));
        assert!(with.contains("shell.active"));
        assert!(with.contains("Enter-WinZshSessionIfActive"));
        assert!(with.contains("PowerShell.OnIdle"));
        assert!(!with.contains("__WinZshStockPrompt"));
        let gone = remove_hook_block(&with).expect("remove");
        assert!(!gone.contains(PROFILE_BEGIN));
        assert!(gone.contains("Write-Host hi"));
    }
}
