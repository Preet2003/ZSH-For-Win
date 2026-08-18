//! Nushell experimental config.nu hook.

use crate::markers::{self, BEGIN, END};
use crate::{Capabilities, ShellHost, ShellId};
use std::path::PathBuf;
use winzsh_core::WinzshPaths;
use winzsh_error::Result;
use winzsh_fs::{atomic_write, ensure_dir, read_string};

/// Nushell host (experimental).
#[derive(Debug, Clone)]
pub struct NuHost {
    /// WinZSH paths.
    pub paths: WinzshPaths,
    /// Override config.nu path (tests).
    pub config_path: Option<PathBuf>,
}

impl NuHost {
    /// Create a Nu host using the default config location.
    pub fn new(paths: WinzshPaths) -> Self {
        Self {
            paths,
            config_path: None,
        }
    }

    fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("nushell")
            .join("config.nu")
    }

    fn render_hook(&self) -> String {
        format!(
            r#"{BEGIN}
# Managed by WinZSH (experimental Nushell bridge) — do not edit by hand.
# Full WinZSH runtime still runs inside PowerShell via zsh-for-win.
def winzsh [] {{
  print "WinZSH: launching nested PowerShell session…"
  ^zsh-for-win
}}
{END}
"#
        )
    }
}

impl ShellHost for NuHost {
    fn name(&self) -> &'static str {
        "nu"
    }

    fn id(&self) -> ShellId {
        ShellId::Nu
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            profile_hooks: true,
            line_editor: true,
            completions: false,
            experimental: true,
        }
    }

    fn module_dir(&self) -> Result<PathBuf> {
        Ok(self.paths.runtime_cache())
    }

    fn profile_path(&self) -> Result<PathBuf> {
        Ok(self
            .config_path
            .clone()
            .unwrap_or_else(Self::default_config_path))
    }

    fn install_hook(&self) -> Result<()> {
        let path = self.profile_path()?;
        if let Some(parent) = path.parent() {
            ensure_dir(parent)?;
        }
        let hook = self.render_hook();
        let new_contents = if path.is_file() {
            let existing = read_string(&path)?;
            markers::upsert(&existing, &hook)
        } else {
            format!("{hook}\n")
        };
        atomic_write(&path, new_contents)?;
        Ok(())
    }

    fn remove_hook(&self) -> Result<()> {
        let path = self.profile_path()?;
        if !path.is_file() {
            return Ok(());
        }
        let existing = read_string(&path)?;
        let next = markers::remove(&existing);
        if next.trim().is_empty() {
            std::fs::remove_file(&path).map_err(|source| winzsh_error::io(path, source))?;
        } else {
            atomic_write(&path, next)?;
        }
        Ok(())
    }

    fn hook_installed(&self) -> Result<bool> {
        let path = self.profile_path()?;
        if !path.is_file() {
            return Ok(false);
        }
        let existing = read_string(&path)?;
        Ok(markers::present(&existing))
    }
}
