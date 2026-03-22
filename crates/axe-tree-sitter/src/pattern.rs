//! Pattern compilation and matching for tree-sitter documents.

use axe_core::language::Language;
use axe_core::match_tree::{PatternNode, MatchStrictness};
use axe_core::meta_var::MetaVariable;
use axe_core::source::SgNode;

use bit_set::BitSet;
use thiserror::Error;
use tree_sitter::Parser;

use crate::node::TsNode;

/// A compiled structural pattern.
#[derive(Clone)]
pub struct TsPattern {
    /// The compiled pattern tree.
    pub node: PatternNode,
    /// Strictness level for matching.
    pub strictness: MatchStrictness,
}

impl TsPattern {
    /// Compile a pattern string using a tree-sitter language.
    pub fn new(
        pattern: &str,
        lang: &impl Language,
        ts_lang: tree_sitter::Language,
    ) -> Result<Self, PatternError> {
        let processed = lang.pre_process_pattern(pattern);
        let mut parser = Parser::new();
        parser
            .set_language(&ts_lang)
            .map_err(|e| PatternError::Language(e.to_string()))?;
        let tree = parser
            .parse(processed.as_bytes(), None)
            .ok_or(PatternError::Timeout)?;

        let src = processed.as_bytes();
        let root_node = tree.root_node();

        if root_node.child_count() == 0 {
            return Err(PatternError::NoContent(pattern.to_string()));
        }

        // Find the single significant top-level node.
        let significant: Vec<_> = (0..root_node.named_child_count() as u32)
            .filter_map(|i| root_node.named_child(i))
            .collect();

        if significant.is_empty() {
            return Err(PatternError::NoContent(pattern.to_string()));
        }
        if significant.len() > 1 {
            return Err(PatternError::MultipleNodes(pattern.to_string()));
        }

        let target = TsNode::new(significant[0], src);
        let expando = lang.expando_char();
        let pattern_node = compile_ts_node(&target, expando);

        Ok(Self {
            node: pattern_node,
            strictness: MatchStrictness::default(),
        })
    }

    /// Set match strictness.
    pub fn with_strictness(mut self, s: MatchStrictness) -> Self {
        self.strictness = s;
        self
    }

    /// Compute the set of node kinds this pattern could match.
    pub fn potential_kinds(&self) -> Option<BitSet> {
        match &self.node {
            PatternNode::MetaVar { .. } => None,
            PatternNode::Terminal { kind_id, .. } | PatternNode::Internal { kind_id, .. } => {
                let mut set = BitSet::new();
                set.insert(*kind_id as usize);
                Some(set)
            }
        }
    }
}

/// Compile a tree-sitter node into a PatternNode, extracting meta-variables.
fn compile_ts_node(node: &TsNode<'_>, expando: char) -> PatternNode {
    let text = node.text();
    if let Some(meta_var) = axe_core::meta_var::extract_meta_var(text, expando) {
        return PatternNode::MetaVar { meta_var };
    }
    if node.is_leaf() {
        return PatternNode::Terminal {
            text: text.to_string(),
            is_named: node.is_named(),
            kind_id: node.kind_id(),
        };
    }
    let children: Vec<PatternNode> = node
        .children()
        .iter()
        .filter(|n| !n.is_error())
        .map(|n| compile_ts_node(n, expando))
        .collect();
    PatternNode::Internal {
        kind_id: node.kind_id(),
        children,
    }
}

/// Errors during pattern compilation.
#[derive(Debug, Error)]
pub enum PatternError {
    #[error("failed to set language: {0}")]
    Language(String),
    #[error("parse timed out")]
    Timeout,
    #[error("pattern has no AST content: `{0}`")]
    NoContent(String),
    #[error("pattern has multiple top-level nodes: `{0}`")]
    MultipleNodes(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use axe_core::meta_var::MetaVariable;
    use axe_language::JavaScript;

    #[test]
    fn compile_simple_pattern() {
        let pat = TsPattern::new(
            "console.log($A)",
            &JavaScript,
            JavaScript::ts_language(),
        ).unwrap();

        // The root should be an Internal node (call_expression or similar).
        match &pat.node {
            PatternNode::Internal { children, .. } => {
                assert!(!children.is_empty(), "should have children");
                // One of the descendants should be a MetaVar for $A.
                fn has_meta_var(node: &PatternNode, name: &str) -> bool {
                    match node {
                        PatternNode::MetaVar { meta_var: MetaVariable::Capture(n, _) } => n == name,
                        PatternNode::Internal { children, .. } => children.iter().any(|c| has_meta_var(c, name)),
                        _ => false,
                    }
                }
                assert!(has_meta_var(&pat.node, "A"), "should contain meta-var $A");
            }
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn compile_pattern_with_ellipsis() {
        let pat = TsPattern::new(
            "console.log($$$ARGS)",
            &JavaScript,
            JavaScript::ts_language(),
        ).unwrap();

        fn has_multi_capture(node: &PatternNode, name: &str) -> bool {
            match node {
                PatternNode::MetaVar { meta_var: MetaVariable::MultiCapture(n) } => n == name,
                PatternNode::Internal { children, .. } => children.iter().any(|c| has_multi_capture(c, name)),
                _ => false,
            }
        }
        assert!(has_multi_capture(&pat.node, "ARGS"), "should contain multi-capture $$$ARGS");
    }

    #[test]
    fn compile_invalid_pattern() {
        let result = TsPattern::new("", &JavaScript, JavaScript::ts_language());
        assert!(result.is_err());
    }

    #[test]
    fn potential_kinds_for_pattern() {
        let pat = TsPattern::new(
            "console.log($A)",
            &JavaScript,
            JavaScript::ts_language(),
        ).unwrap();
        let kinds = pat.potential_kinds();
        assert!(kinds.is_some(), "concrete pattern should have potential kinds");
        assert!(kinds.unwrap().len() > 0);
    }
}
