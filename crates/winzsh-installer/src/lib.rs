//! Detect, backup, install, uninstall, and verify WinZSH (idempotent).

#![forbid(unsafe_code)]

/// Outcome of an install/uninstall/verify operation.
#[derive(Debug, Clone, Default)]
pub struct InstallReport {
    /// Human-readable steps performed or skipped.
    pub steps: Vec<String>,
}
