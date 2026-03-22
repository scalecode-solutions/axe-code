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

/// A test case for a rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[forma(default)]
pub struct RuleTest {
    /// Code snippets that SHOULD match the rule.
    #[forma(default)]
    pub valid: Vec<String>,
    /// Code snippets that should NOT match the rule (i.e., they violate it).
    #[forma(default)]
    pub invalid: Vec<String>,
}

impl Default for RuleTest {
    fn default() -> Self {
        Self { valid: Vec::new(), invalid: Vec::new() }
    }
}

/// Complete rule configuration from a rule file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[forma(default)]
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
    /// Test cases for this rule.
    #[forma(default)]
    pub tests: Option<RuleTest>,
}

/// Project-level configuration (axeconfig.json).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[forma(default)]
pub struct ProjectConfig {
    /// Directories containing rule files.
    #[forma(default)]
    pub rule_dirs: Vec<String>,
    /// Individual rule files.
    #[forma(default)]
    pub rules: Vec<String>,
}

impl ProjectConfig {
    /// Try to find and load axeconfig.json by walking up from the given path.
    pub fn discover(start: &std::path::Path) -> Option<(std::path::PathBuf, Self)> {
        let mut dir = if start.is_file() {
            start.parent()?.to_path_buf()
        } else {
            start.to_path_buf()
        };
        loop {
            let config_path = dir.join("axeconfig.json");
            if config_path.exists() {
                let content = std::fs::read_to_string(&config_path).ok()?;
                let config: ProjectConfig = forma_json::from_str(&content).ok()?;
                return Some((dir, config));
            }
            if !dir.pop() {
                return None;
            }
        }
    }
}
