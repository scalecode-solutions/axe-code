//! Combined scan engine with kind-based dispatch.
//!
//! Builds a `kind_id → [rule_index]` mapping and does a single DFS traversal,
//! dispatching to only the rules whose `potential_kinds` include the current
//! node's kind. This is the performance backbone of `axe scan`.

use axe_core::node::{Node, NodeMatch};
use axe_core::source::Doc;

use crate::rule::Rule;
use crate::rule_config::{RuleConfig, Severity};

/// A scan hit: rule index + matched node with captures.
pub struct ScanHit<'tree, D: Doc> {
    pub rule_idx: usize,
    pub node_match: NodeMatch<'tree, D>,
}

/// Combined multi-rule scanner with kind-based dispatch.
///
/// Instead of running every rule against every node (O(rules * nodes)),
/// this builds a dispatch table so each node only checks the rules that
/// could possibly match its kind (O(nodes * avg_rules_per_kind)).
pub struct CombinedScan {
    /// Compiled rules with their configs.
    rules: Vec<CompiledRuleEntry>,
    /// kind_id → Vec<rule_index>. Sparse: index by kind_id.
    kind_map: Vec<Vec<usize>>,
    /// Rules that match any kind (no potential_kinds constraint).
    any_kind_rules: Vec<usize>,
}

struct CompiledRuleEntry {
    rule: Rule,
    id: String,
    severity: Severity,
    message: Option<String>,
    fix: Option<String>,
}

impl CombinedScan {
    /// Build from a list of (compiled rule, config) pairs.
    pub fn new(entries: Vec<(Rule, &RuleConfig)>) -> Self {
        let mut rules = Vec::new();
        let mut kind_map: Vec<Vec<usize>> = Vec::new();
        let mut any_kind_rules = Vec::new();

        for (idx, (rule, config)) in entries.into_iter().enumerate() {
            match rule.potential_kinds() {
                Some(kinds) => {
                    for kind in kinds.iter() {
                        while kind_map.len() <= kind {
                            kind_map.push(Vec::new());
                        }
                        kind_map[kind].push(idx);
                    }
                }
                None => {
                    any_kind_rules.push(idx);
                }
            }

            rules.push(CompiledRuleEntry {
                rule,
                id: config.id.clone(),
                severity: config.severity.unwrap_or_default(),
                message: config.message.clone(),
                fix: config.fix.clone(),
            });
        }

        Self { rules, kind_map, any_kind_rules }
    }

    /// Scan a parsed document, returning all hits.
    ///
    /// If `src` is provided, suppression comments (`axe-ignore` / `axe-ignore-next-line`)
    /// on the line above a match will suppress that hit.
    pub fn scan<'tree, D: Doc>(
        &self,
        root: &Node<'tree, D>,
        src: &str,
    ) -> Vec<ScanHit<'tree, D>> {
        let mut hits = Vec::new();

        for node in root.dfs() {
            let kind = node.kind_id() as usize;

            // Check rules mapped to this kind.
            if let Some(rule_indices) = self.kind_map.get(kind) {
                for &idx in rule_indices {
                    if let Some(m) = self.rules[idx].rule.match_node(node.clone()) {
                        let node_line = m.node().start_pos().line;
                        if is_suppressed(src, node_line, &self.rules[idx].id) {
                            continue;
                        }
                        hits.push(ScanHit { rule_idx: idx, node_match: m });
                    }
                }
            }

            // Check rules that match any kind.
            for &idx in &self.any_kind_rules {
                if let Some(m) = self.rules[idx].rule.match_node(node.clone()) {
                    let node_line = m.node().start_pos().line;
                    if is_suppressed(src, node_line, &self.rules[idx].id) {
                        continue;
                    }
                    hits.push(ScanHit { rule_idx: idx, node_match: m });
                }
            }
        }

        hits
    }

    /// Get the rule ID for a hit.
    pub fn rule_id(&self, idx: usize) -> &str {
        &self.rules[idx].id
    }

    /// Get the severity for a rule.
    pub fn severity(&self, idx: usize) -> Severity {
        self.rules[idx].severity
    }

    /// Get the message for a rule.
    pub fn message(&self, idx: usize) -> Option<&str> {
        self.rules[idx].message.as_deref()
    }

    /// Get the fix template for a rule.
    pub fn fix(&self, idx: usize) -> Option<&str> {
        self.rules[idx].fix.as_deref()
    }

    /// Number of rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

/// Check if a hit at the given 0-indexed line is suppressed by a comment on the line above.
///
/// Supports `axe-ignore` (suppress all rules) and `axe-ignore <rule-id>` (suppress specific rule).
/// Also supports `axe-ignore-next-line` as an alias.
/// Recognizes comment prefixes: `//`, `#`, `--`, `/*`, `<!--`.
fn is_suppressed(src: &str, line: usize, rule_id: &str) -> bool {
    if line == 0 {
        return false;
    }
    let lines: Vec<&str> = src.lines().collect();
    let prev_line = if line <= lines.len() {
        lines[line - 1].trim()
    } else {
        return false;
    };

    for prefix in ["//", "#", "--", "/*", "<!--"] {
        if let Some(rest) = prev_line.strip_prefix(prefix) {
            let rest = rest.trim();
            // Strip trailing comment closers for block-style comments.
            let rest = rest.trim_end_matches("*/").trim_end_matches("-->").trim();

            if rest == "axe-ignore" || rest == "axe-ignore-next-line" {
                return true; // Suppress all rules
            }
            if let Some(ids) = rest
                .strip_prefix("axe-ignore ")
                .or_else(|| rest.strip_prefix("axe-ignore-next-line "))
            {
                return ids.split(',').map(|s| s.trim()).any(|id| id == rule_id);
            }
        }
    }
    false
}
