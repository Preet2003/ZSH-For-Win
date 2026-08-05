//! Sync module facade — future phase. Intentionally empty of product behavior.

#![forbid(unsafe_code)]

/// Marker that the sync crate is present in the workspace for future wiring.
pub const PHASE: &str = "future-sync";
