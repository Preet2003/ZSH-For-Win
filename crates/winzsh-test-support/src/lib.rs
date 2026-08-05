//! Test fixtures: temp homes, golden helpers, fake PATH, sample manifests.

#![forbid(unsafe_code)]

use std::path::PathBuf;

/// Create a temporary directory path name for tests (IO helpers land with Phase 1).
pub fn temp_home_name(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("winzsh-test-{prefix}"))
}
