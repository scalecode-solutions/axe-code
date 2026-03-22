//! Match strictness — controls which nodes participate in matching.

use crate::meta_var::MetaVarEnv;
use crate::node::Node;
use crate::source::Doc;

/// Controls which AST nodes participate in pattern matching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchStrictness {
    /// All nodes, including punctuation and whitespace tokens.
    Cst,
    /// All nodes except source trivia (extra whitespace). Default.
    Smart,
    /// Named nodes only (unnamed punctuation is skipped).
    Ast,
    /// Named nodes, excluding comments.
    Relaxed,
    /// Named nodes, text comparison ignored (structure-only matching).
    Signature,
    /// Text-only matching — node kinds are ignored, only text matters.
    Template,
}

impl Default for MatchStrictness {
    fn default() -> Self {
        MatchStrictness::Smart
    }
}

/// Result of comparing a single pattern node against a single AST node.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MatchOneNode {
    /// Both pattern and candidate matched.
    MatchedBoth,
    /// Skip both pattern and candidate (both are trivial).
    SkipBoth,
    /// Skip the pattern node (it's trivial but candidate isn't).
    SkipGoal,
    /// Skip the candidate node (it's trivial but pattern isn't).
    SkipCandidate,
    /// Hard mismatch — matching fails.
    NoMatch,
}

fn is_comment(node: &Node<impl Doc>) -> bool {
    node.kind().contains("comment")
}

impl MatchStrictness {
    /// Whether kind comparison should be skipped entirely (Template mode).
    pub fn should_skip_kind(&self) -> bool {
        matches!(self, MatchStrictness::Template)
    }

    /// Whether a candidate should be skipped when matching against a meta-variable.
    pub fn should_skip_candidate_for_metavar<D: Doc>(&self, candidate: &Node<'_, D>) -> bool {
        match self {
            MatchStrictness::Cst => false,
            MatchStrictness::Smart => false,
            MatchStrictness::Ast | MatchStrictness::Relaxed | MatchStrictness::Signature => {
                !candidate.is_named()
            }
            MatchStrictness::Template => !candidate.is_named(),
        }
    }

    /// Whether a trailing candidate node (after all goals matched) should be skipped.
    pub fn should_skip_trailing<D: Doc>(&self, node: &Node<'_, D>) -> bool {
        match self {
            MatchStrictness::Cst => false,
            MatchStrictness::Smart => !node.is_named(),
            MatchStrictness::Ast => !node.is_named(),
            MatchStrictness::Relaxed => !node.is_named() || is_comment(node),
            MatchStrictness::Signature => !node.is_named() || is_comment(node),
            MatchStrictness::Template => !node.is_named(),
        }
    }

    /// Compare a terminal pattern node against a candidate AST node.
    pub fn match_terminal<'tree, D: Doc>(
        &self,
        is_named: bool,
        text: &str,
        goal_kind: u16,
        candidate: &Node<'tree, D>,
        _env: &mut MetaVarEnv<'tree, D>,
    ) -> MatchOneNode {
        let cand_kind = candidate.kind_id();
        let is_kind_matched = super::are_kinds_matching(goal_kind, cand_kind);

        // For unnamed nodes, kind match alone is sufficient (text can differ
        // due to tree-sitter bugs with unnamed node spans).
        if is_kind_matched && (!is_named || text == candidate.text()) {
            return MatchOneNode::MatchedBoth;
        }

        // Comment skipping.
        if self.should_skip_comment() && is_comment(candidate) {
            return MatchOneNode::SkipCandidate;
        }

        let (skip_goal, skip_candidate) = match self {
            MatchStrictness::Cst => (false, false),
            MatchStrictness::Smart => (false, !candidate.is_named()),
            MatchStrictness::Ast | MatchStrictness::Relaxed => {
                (!is_named, !candidate.is_named())
            }
            MatchStrictness::Signature => {
                if is_kind_matched {
                    return MatchOneNode::MatchedBoth;
                }
                (!is_named, !candidate.is_named())
            }
            MatchStrictness::Template => {
                if text == candidate.text() {
                    return MatchOneNode::MatchedBoth;
                }
                (false, !candidate.is_named())
            }
        };

        match (skip_goal, skip_candidate) {
            (true, true) => MatchOneNode::SkipBoth,
            (true, false) => MatchOneNode::SkipGoal,
            (false, true) => MatchOneNode::SkipCandidate,
            (false, false) => MatchOneNode::NoMatch,
        }
    }

    fn should_skip_comment(&self) -> bool {
        matches!(
            self,
            MatchStrictness::Relaxed | MatchStrictness::Signature | MatchStrictness::Template
        )
    }
}
