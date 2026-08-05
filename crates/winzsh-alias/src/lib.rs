//! Alias model and deterministic conflict resolution.

#![forbid(unsafe_code)]

/// A single shell alias.
#[derive(Debug, Clone)]
pub struct Alias {
    /// Alias name.
    pub name: String,
    /// Expansion body.
    pub value: String,
}
