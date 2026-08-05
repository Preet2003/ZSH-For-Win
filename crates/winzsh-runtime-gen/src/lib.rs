//! Single writer of runtime artifacts under `~/.winzsh/cache/runtime/`.

#![forbid(unsafe_code)]

/// Hash/lockfile summary of inputs used to decide whether regen is needed.
#[derive(Debug, Clone, Default)]
pub struct RuntimeLock {
    /// Opaque content hash of generation inputs.
    pub input_hash: String,
}
