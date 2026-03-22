//! Top-level rule configuration.

use forma_derive::{Deserialize, Serialize};
use crate::rule::SerializableRule;

/// Severity level for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Severity {
    Hint,
    Info,
    #[default]
    Warning,
    Error,
}

/// Complete rule configuration from a rule file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleConfig {
    pub id: String,
    pub language: String,
    pub rule: SerializableRule,
    pub severity: Option<Severity>,
    pub message: Option<String>,
    pub note: Option<String>,
    pub fix: Option<String>,
    pub url: Option<String>,
}
