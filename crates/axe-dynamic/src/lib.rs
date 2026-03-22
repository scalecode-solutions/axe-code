//! Dynamic tree-sitter language loading for axe.
//!
//! Loads tree-sitter grammars from shared libraries at runtime,
//! enabling support for languages not built into the axe binary.

use std::path::PathBuf;
use thiserror::Error;

/// Errors from dynamic language loading.
#[derive(Debug, Error)]
pub enum DynamicLangError {
    #[error("dynamic languages not configured")]
    NotConfigured,
    #[error("failed to open library: {0}")]
    OpenLib(String),
    #[error("failed to read symbol: {0}")]
    ReadSymbol(String),
    #[error("incompatible tree-sitter version: {0}")]
    IncompatibleVersion(usize),
}

/// A dynamically loaded language.
pub struct DynamicLang {
    // Placeholder — will be implemented with libloading
    _path: PathBuf,
}
