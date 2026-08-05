//! Theme package resolve, validate, and install contracts.

#![forbid(unsafe_code)]

use winzsh_core::ThemeId;

/// Resolved active theme.
#[derive(Debug, Clone)]
pub struct ResolvedTheme {
    /// Theme identifier.
    pub id: ThemeId,
}
