//! Diagnostics and remediation hints consumed by CLI and installer verify.

#![forbid(unsafe_code)]

/// Severity of a diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Informational.
    Info,
    /// Warning; shell may still work.
    Warning,
    /// Error; user action required.
    Error,
}

/// One structured diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity.
    pub severity: Severity,
    /// Stable machine code.
    pub code: String,
    /// Human message.
    pub message: String,
}
