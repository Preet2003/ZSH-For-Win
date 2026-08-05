//! Test fixtures: temp homes, golden helpers, fake PATH, sample manifests.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use winzsh_core::WinzshPaths;
use winzsh_fs::ensure_layout;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Temporary WinZSH home for tests.
#[derive(Debug)]
pub struct TempHome {
    /// Root directory that will be deleted on drop.
    pub root: PathBuf,
    /// Path helpers for the temp home.
    pub paths: WinzshPaths,
}

impl TempHome {
    /// Create a unique temporary home and ensure layout directories.
    pub fn new(prefix: &str) -> Self {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("winzsh-test-{prefix}-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let paths = WinzshPaths::from_root(root.clone());
        if let Err(err) = ensure_layout(&paths) {
            panic!("failed to create temp WinZSH layout: {err}");
        }
        Self { root, paths }
    }

    /// Profile path under this temp home (does not touch the real user profile).
    pub fn profile_path(&self) -> PathBuf {
        self.root.join("Microsoft.PowerShell_profile.ps1")
    }
}

impl Drop for TempHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Create a temporary directory path name for tests.
pub fn temp_home_name(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("winzsh-test-{prefix}"))
}
