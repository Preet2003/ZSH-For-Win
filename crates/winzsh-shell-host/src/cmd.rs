//! CMD launcher integration (no rich profile; uses `zsh-for-win.cmd`).

use crate::{Capabilities, ShellHost, ShellId};
use std::path::PathBuf;
use winzsh_core::WinzshPaths;
use winzsh_error::Result;
use winzsh_fs::{atomic_write, ensure_dir};

/// CMD host — ensures the WinZSH launcher exists under `~/.winzsh/bin`.
#[derive(Debug, Clone)]
pub struct CmdHost {
    /// WinZSH paths.
    pub paths: WinzshPaths,
}

impl CmdHost {
    /// Create a CMD host.
    pub fn new(paths: WinzshPaths) -> Self {
        Self { paths }
    }

    fn launcher_path(&self) -> PathBuf {
        self.paths.bin_dir().join("zsh-for-win.cmd")
    }

    fn render_launcher(&self) -> String {
        concat!(
            "@echo off\r\n",
            "REM Managed by WinZSH — nested session. Type \"exit\" to return.\r\n",
            "set WINZSH_SHELL=1\r\n",
            "where pwsh >nul 2>&1\r\n",
            "if errorlevel 1 (\r\n",
            "  powershell %*\r\n",
            "  exit /b %ERRORLEVEL%\r\n",
            ")\r\n",
            "pwsh %*\r\n",
        )
        .into()
    }
}

impl ShellHost for CmdHost {
    fn name(&self) -> &'static str {
        "cmd"
    }

    fn id(&self) -> ShellId {
        ShellId::Cmd
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            profile_hooks: false,
            line_editor: false,
            completions: false,
            experimental: false,
        }
    }

    fn module_dir(&self) -> Result<PathBuf> {
        Ok(self.paths.bin_dir())
    }

    fn profile_path(&self) -> Result<PathBuf> {
        Ok(self.launcher_path())
    }

    fn install_hook(&self) -> Result<()> {
        ensure_dir(&self.paths.bin_dir())?;
        atomic_write(&self.launcher_path(), self.render_launcher())?;
        Ok(())
    }

    fn remove_hook(&self) -> Result<()> {
        let path = self.launcher_path();
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|source| winzsh_error::io(path.clone(), source))?;
        }
        Ok(())
    }

    fn hook_installed(&self) -> Result<bool> {
        Ok(self.launcher_path().is_file())
    }
}
