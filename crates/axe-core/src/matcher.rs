//! The [`Matcher`] trait — composable AST matching interface.

use bit_set::BitSet;

use crate::meta_var::MetaVarEnv;
use crate::node::Node;
use crate::source::Doc;

// ---------------------------------------------------------------------------
// Matcher trait
// ---------------------------------------------------------------------------

/// Core matching trait. All rules implement this.
///
/// The `potential_kinds` method is critical for performance: it returns the
/// set of node kinds that could possibly match, enabling the scan engine to
/// skip entire subtrees.
pub trait Matcher {
    /// Try to match `node`. On success, returns the matched node.
    fn match_node_with_env<'tree, D: Doc>(
        &self,
        node: Node<'tree, D>,
        env: &mut MetaVarEnv<'tree, D>,
    ) -> Option<Node<'tree, D>>;

    /// The set of node kind IDs that this matcher could possibly match.
    /// Returns `None` if the matcher can match any kind.
    fn potential_kinds(&self) -> Option<BitSet> {
        None
    }

    /// Number of nodes consumed by this match.
    fn get_match_len<D: Doc>(&self, _node: Node<'_, D>) -> Option<usize> {
        None
    }
}

// ---------------------------------------------------------------------------
// MatcherExt
// ---------------------------------------------------------------------------

/// Convenience methods for matchers.
pub trait MatcherExt: Matcher {
    fn matches_node<D: Doc>(&self, node: Node<'_, D>) -> bool {
        let mut env = MetaVarEnv::new();
        self.match_node_with_env(node, &mut env).is_some()
    }
}

impl<M: Matcher + ?Sized> MatcherExt for M {}

// ---------------------------------------------------------------------------
// Built-in matchers
// ---------------------------------------------------------------------------

/// Matches every node.
pub struct MatchAll;

impl Matcher for MatchAll {
    fn match_node_with_env<'tree, D: Doc>(
        &self,
        node: Node<'tree, D>,
        _env: &mut MetaVarEnv<'tree, D>,
    ) -> Option<Node<'tree, D>> {
        Some(node)
    }
}

/// Matches no node.
pub struct MatchNone;

impl Matcher for MatchNone {
    fn match_node_with_env<'tree, D: Doc>(
        &self,
        _node: Node<'tree, D>,
        _env: &mut MetaVarEnv<'tree, D>,
    ) -> Option<Node<'tree, D>> {
        None
    }

    fn potential_kinds(&self) -> Option<BitSet> {
        Some(BitSet::new())
    }
}

/// Matches nodes of a specific kind.
#[derive(Clone, Debug)]
pub struct KindMatcher {
    kind_id: u16,
}

impl KindMatcher {
    pub fn from_id(kind_id: u16) -> Self {
        Self { kind_id }
    }
}

impl Matcher for KindMatcher {
    fn match_node_with_env<'tree, D: Doc>(
        &self,
        node: Node<'tree, D>,
        _env: &mut MetaVarEnv<'tree, D>,
    ) -> Option<Node<'tree, D>> {
        if node.kind_id() == self.kind_id {
            Some(node)
        } else {
            None
        }
    }

    fn potential_kinds(&self) -> Option<BitSet> {
        let mut set = BitSet::new();
        set.insert(self.kind_id as usize);
        Some(set)
    }
}

/// Matches nodes whose text matches a regex.
#[derive(Clone, Debug)]
pub struct RegexMatcher {
    pattern: regex::Regex,
}

impl RegexMatcher {
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self { pattern: regex::Regex::new(pattern)? })
    }
}

impl Matcher for RegexMatcher {
    fn match_node_with_env<'tree, D: Doc>(
        &self,
        node: Node<'tree, D>,
        _env: &mut MetaVarEnv<'tree, D>,
    ) -> Option<Node<'tree, D>> {
        if self.pattern.is_match(node.text()) {
            Some(node)
        } else {
            None
        }
    }
}

/// Structural pattern (placeholder — implemented in axe-tree-sitter).
#[derive(Clone, Debug)]
pub struct Pattern {
    _private: (),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_all_has_no_potential_kinds() {
        assert!(MatchAll.potential_kinds().is_none());
    }

    #[test]
    fn match_none_has_empty_potential_kinds() {
        let kinds = MatchNone.potential_kinds().unwrap();
        assert_eq!(kinds.len(), 0);
    }

    #[test]
    fn kind_matcher_potential_kinds() {
        let m = KindMatcher::from_id(42);
        let kinds = m.potential_kinds().unwrap();
        assert!(kinds.contains(42));
        assert_eq!(kinds.len(), 1);
    }
}
