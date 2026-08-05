//! Prompt segment contracts and timing budgets (Rust side / codegen inputs).

#![forbid(unsafe_code)]

/// Identifier for a prompt segment contributed to runtime-gen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentId(pub String);
