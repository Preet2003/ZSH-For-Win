//! Suggestion sources, history policy, and syntax-highlight token rules.

#![forbid(unsafe_code)]

/// High-level suggestion engine policy (executed by the PS runtime).
#[derive(Debug, Clone, Default)]
pub struct SuggestPolicy {
    /// Whether history-based suggestions are enabled.
    pub history_enabled: bool,
}
