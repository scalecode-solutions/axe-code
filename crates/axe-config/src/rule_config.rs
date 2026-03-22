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
#[forma(deny_unknown_fields)]
pub struct RuleConfig {
    pub id: String,
    pub language: String,
    pub rule: SerializableRule,
    #[forma(default)]
    pub severity: Option<Severity>,
    #[forma(default)]
    pub message: Option<String>,
    #[forma(default)]
    pub note: Option<String>,
    #[forma(default)]
    pub fix: Option<String>,
    #[forma(default)]
    pub url: Option<String>,
}
