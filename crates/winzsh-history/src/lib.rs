//! History store schema and query API contracts.

#![forbid(unsafe_code)]

/// One recorded command invocation.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Command line text.
    pub command: String,
}
