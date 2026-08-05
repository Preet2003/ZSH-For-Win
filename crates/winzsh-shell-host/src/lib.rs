//! Shell host abstraction for PowerShell today and additional shells later.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use winzsh_error::Result;

/// Capabilities advertised by a shell integration backend.
#[derive(Debug, Clone, Default)]
pub struct Capabilities {
    /// Whether the host supports managed profile hooks.
    pub profile_hooks: bool,
    /// Whether PSReadLine-style line editing is available.
    pub line_editor: bool,
    /// Whether completions can be registered dynamically.
    pub completions: bool,
}

/// Integration surface implemented per shell (PowerShell, later CMD/Git Bash/Nushell).
pub trait ShellHost {
    /// Human-readable shell name.
    fn name(&self) -> &'static str;

    /// Reported capabilities.
    fn capabilities(&self) -> Capabilities;

    /// Directory where the host expects modules/scripts.
    fn module_dir(&self) -> Result<PathBuf>;

    /// Path to the managed profile file.
    fn profile_path(&self) -> Result<PathBuf>;

    /// Install the managed profile hook.
    fn install_hook(&self) -> Result<()>;

    /// Remove the managed profile hook.
    fn remove_hook(&self) -> Result<()>;

    /// Whether the managed hook is currently present.
    fn hook_installed(&self) -> Result<bool>;
}
