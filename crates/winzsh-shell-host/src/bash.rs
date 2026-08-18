//! Git Bash / bash experimental `.bashrc` hook.

use crate::markers::{self, BEGIN, END};
use crate::{Capabilities, ShellHost, ShellId};
use std::path::PathBuf;
use winzsh_core::WinzshPaths;
use winzsh_error::Result;
use winzsh_fs::{atomic_write, ensure_dir, read_string};

/// Bash host (experimental).
#[derive(Debug, Clone)]
pub struct BashHost {
    /// WinZSH paths.
    pub paths: WinzshPaths,
    /// Override bashrc path (tests).
    pub bashrc_path: Option<PathBuf>,
}

impl BashHost {
    /// Create a Bash host.
    pub fn new(paths: WinzshPaths) -> Self {
        Self {
            paths,
            bashrc_path: None,
        }
    }

    fn default_bashrc() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".bashrc")
    }

    fn render_hook(&self) -> String {
        format!(
            r#"{BEGIN}
# Managed by WinZSH (experimental bash bridge) — do not edit by hand.
winzsh() {{
  echo "WinZSH: launching nested PowerShell session…"
  zsh-for-win "$@"
}}
{END}
"#
        )
    }
}

impl ShellHost for BashHost {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn id(&self) -> ShellId {
        ShellId::Bash
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
        Ok(self.paths.bin_dir())
    }

    fn profile_path(&self) -> Result<PathBuf> {
        Ok(self
            .bashrc_path
            .clone()
            .unwrap_or_else(Self::default_bashrc))
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
