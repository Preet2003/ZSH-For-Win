//! Self-update, channel selection, checksum verify, and rollback metadata.

#![forbid(unsafe_code)]

use winzsh_core::Channel;

/// Update check request (network implementation lands with Phase 1 update work).
#[derive(Debug, Clone)]
pub struct UpdateCheck {
    /// Channel to query.
    pub channel: Channel,
}
