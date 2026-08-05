//! Tool and environment detection for lazy feature enablement.

#![forbid(unsafe_code)]

/// Placeholder detection result.
#[derive(Debug, Clone, Default)]
pub struct DetectionReport {
    /// Names of detected commands on PATH.
    pub commands: Vec<String>,
}
