//! Completion pack catalog and lazy-load rules.

#![forbid(unsafe_code)]

/// Descriptor for a completion pack materialized into the runtime module.
#[derive(Debug, Clone)]
pub struct CompletionPack {
    /// Command that triggers lazy load.
    pub command: String,
}
