//! Language Server Protocol implementation for axe.
//!
//! ## Improvements over ast-grep's LSP
//!
//! - **Incremental parsing**: Uses tree-sitter's `tree.edit()` + incremental
//!   re-parse instead of full re-parse on every change.
//! - **Structured logging**: Uses `tracing` instead of silent failures.
//! - **Proper error propagation**: Rule loading errors are reported to the
//!   client, not swallowed.

// Placeholder — will be implemented in Phase 4.
