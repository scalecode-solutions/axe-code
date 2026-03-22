//! Core pattern-to-AST matching algorithm.
//!
//! Compares a [`PatternNode`] tree against a live AST. Supports:
//! - Terminal matching (leaf text comparison)
//! - Internal node matching (recursive structural comparison)
//! - Meta-variable capture (`$VAR`, `$$$VAR`, `$_`)
//! - Ellipsis matching (`$$$` — zero or more nodes)
//! - Match strictness levels

pub mod strictness;

use crate::meta_var::{MetaVarEnv, MetaVariable};
use crate::node::Node;
use crate::source::Doc;

pub use strictness::{MatchOneNode, MatchStrictness};

// ---------------------------------------------------------------------------
// PatternNode
// ---------------------------------------------------------------------------

/// A node in a compiled pattern tree.
#[derive(Clone, Debug)]
pub enum PatternNode {
    /// A meta-variable (`$VAR`, `$$$ARGS`, `$_`, `$$$`).
    MetaVar { meta_var: MetaVariable },
    /// A leaf node — compared by text and optionally kind.
    Terminal { text: String, is_named: bool, kind_id: u16 },
    /// An internal node — compared by kind, then children recursively.
    Internal { kind_id: u16, children: Vec<PatternNode> },
}

impl PatternNode {
    pub fn is_ellipsis(&self) -> bool {
        matches!(
            self,
            PatternNode::MetaVar { meta_var: MetaVariable::Ellipsis | MetaVariable::MultiCapture(_) }
        )
    }

    /// Trivial = unnamed terminal (punctuation, keywords). Skipped after ellipsis.
    pub fn is_trivial(&self) -> bool {
        matches!(self, PatternNode::Terminal { is_named: false, .. })
    }
}

// ---------------------------------------------------------------------------
// Kind utilities
// ---------------------------------------------------------------------------

/// tree-sitter's built-in ERROR kind ID is always 0xFFFF.
const TS_ERROR_KIND: u16 = 0xFFFF;

/// Check if two kind IDs match. An ERROR kind in the pattern matches any kind
/// (useful for `kind: ERROR` rules that find parse errors).
#[inline]
fn are_kinds_matching(goal: u16, candidate: u16) -> bool {
    goal == candidate || goal == TS_ERROR_KIND
}

// ---------------------------------------------------------------------------
// Matching algorithm
// ---------------------------------------------------------------------------

/// Match a single pattern node against a single AST node.
///
/// Returns a `MatchOneNode` indicating whether matching succeeded, or whether
/// one or both sides should be skipped (for strictness-based filtering).
pub fn match_node_impl<'tree, D: Doc>(
    goal: &PatternNode,
    candidate: &Node<'tree, D>,
    env: &mut MetaVarEnv<'tree, D>,
    strictness: &MatchStrictness,
) -> MatchOneNode {
    match goal {
        PatternNode::Terminal { text, kind_id, is_named } => {
            strictness.match_terminal(*is_named, text, *kind_id, candidate, env)
        }
        PatternNode::MetaVar { meta_var } => {
            if strictness.should_skip_candidate_for_metavar(candidate) {
                return MatchOneNode::SkipCandidate;
            }
            match match_leaf_meta_var(meta_var, candidate, env) {
                Some(()) => MatchOneNode::MatchedBoth,
                None => MatchOneNode::NoMatch,
            }
        }
        PatternNode::Internal { kind_id, children } => {
            let kind_matched = strictness.should_skip_kind()
                || are_kinds_matching(*kind_id, candidate.kind_id());
            if !kind_matched {
                return MatchOneNode::NoMatch;
            }
            let cand_children: Vec<_> = candidate.children().collect();
            match match_children(children, cand_children.into_iter(), env, strictness) {
                Some(()) => MatchOneNode::MatchedBoth,
                None => MatchOneNode::NoMatch,
            }
        }
    }
}

/// Match a meta-variable against a candidate node, updating the environment.
fn match_leaf_meta_var<'tree, D: Doc>(
    mv: &MetaVariable,
    candidate: &Node<'tree, D>,
    env: &mut MetaVarEnv<'tree, D>,
) -> Option<()> {
    match mv {
        MetaVariable::Capture(name, named) => {
            if *named && !candidate.is_named() {
                return None;
            }
            if env.insert_single(name, candidate.clone()) {
                Some(())
            } else {
                None
            }
        }
        MetaVariable::Anonymous(named) => {
            if *named && !candidate.is_named() {
                None
            } else {
                Some(())
            }
        }
        MetaVariable::MultiCapture(name) => {
            // Single-node context: capture as single.
            if env.insert_single(name, candidate.clone()) {
                Some(())
            } else {
                None
            }
        }
        MetaVariable::Ellipsis => {
            // Bare ellipsis in leaf position — just accept.
            Some(())
        }
    }
}

/// Match a sequence of pattern children against a sequence of candidate children.
///
/// This is the core loop. It handles ellipsis matching (greedy with lookahead),
/// trivial-node skipping, and strictness-based filtering.
fn match_children<'tree, D: Doc>(
    goals: &[PatternNode],
    candidates: impl Iterator<Item = Node<'tree, D>>,
    env: &mut MetaVarEnv<'tree, D>,
    strictness: &MatchStrictness,
) -> Option<()> {
    let mut goal_iter = goals.iter().peekable();
    let mut cand_iter: std::iter::Peekable<_> = candidates.peekable();

    // Empty pattern children: only match if all candidates are skippable.
    if goal_iter.peek().is_none() {
        // An internal node with no pattern children matches if candidates
        // are all trivial/skippable. This matches ast-grep behavior for
        // patterns like `f()` matching `f(/* comment */)` in Relaxed mode.
        while let Some(c) = cand_iter.peek() {
            if strictness.should_skip_trailing(c) {
                cand_iter.next();
            } else {
                return None;
            }
        }
        return Some(());
    }

    loop {
        // Check if current goal is an ellipsis.
        if let Some(goal) = goal_iter.peek() {
            if goal.is_ellipsis() {
                let ellipsis_name = match &goal {
                    PatternNode::MetaVar { meta_var: MetaVariable::MultiCapture(n) } => Some(n.as_str()),
                    _ => None,
                };
                goal_iter.next();

                // Skip trivial nodes in remaining goals.
                while goal_iter.peek().is_some_and(|g| g.is_trivial()) {
                    goal_iter.next();
                }

                // If no more goals, ellipsis consumes all remaining candidates.
                if goal_iter.peek().is_none() {
                    let collected: Vec<_> = cand_iter.collect();
                    if let Some(name) = ellipsis_name {
                        env.insert_multi(name.to_string(), collected);
                    }
                    return Some(());
                }

                // Consecutive ellipsis: consume one candidate.
                if goal_iter.peek().is_some_and(|g| g.is_ellipsis()) {
                    let node = cand_iter.next()?;
                    if let Some(name) = ellipsis_name {
                        env.insert_multi(name.to_string(), vec![node]);
                    }
                    continue;
                }

                // Greedy match: consume candidates until the next goal matches.
                let mut collected = Vec::new();
                loop {
                    let cand = cand_iter.peek()?;
                    // Snapshot env, try matching next goal.
                    let mut trial_env = env.clone();
                    if matches!(
                        match_node_impl(goal_iter.peek().unwrap(), cand, &mut trial_env, strictness),
                        MatchOneNode::MatchedBoth
                    ) {
                        // Found the anchor. Commit ellipsis capture.
                        if let Some(name) = ellipsis_name {
                            env.insert_multi(name.to_string(), collected);
                        }
                        // Restore env from trial (it has the anchor's captures).
                        *env = trial_env;
                        break;
                    }
                    collected.push(cand_iter.next().unwrap());
                }
                continue;
            }
        }

        // Non-ellipsis goal: try to match with current candidate.
        let Some(goal) = goal_iter.peek() else {
            // Goals exhausted — check if remaining candidates are all trailing.
            return cand_iter.all(|n| strictness.should_skip_trailing(&n)).then_some(());
        };

        loop {
            let Some(cand) = cand_iter.peek() else {
                // Candidates exhausted but goals remain.
                // Check if remaining goals are all skippable.
                return goal_iter.all(|g| g.is_trivial()).then_some(());
            };

            match match_node_impl(goal, cand, env, strictness) {
                MatchOneNode::MatchedBoth => {
                    goal_iter.next();
                    cand_iter.next();
                    break;
                }
                MatchOneNode::SkipGoal => {
                    goal_iter.next();
                    break;
                }
                MatchOneNode::SkipCandidate => {
                    cand_iter.next();
                }
                MatchOneNode::SkipBoth => {
                    goal_iter.next();
                    cand_iter.next();
                    break;
                }
                MatchOneNode::NoMatch => return None,
            }
        }

        if goal_iter.peek().is_none() {
            // All goals matched. Check remaining candidates.
            let all_trailing = cand_iter.all(|n| strictness.should_skip_trailing(&n));
            return all_trailing.then_some(());
        }
    }
}

/// Top-level: try to match a pattern against a candidate node.
/// Returns `true` if the pattern matches, populating `env` with captures.
pub fn match_pattern<'tree, D: Doc>(
    pattern: &PatternNode,
    candidate: &Node<'tree, D>,
    env: &mut MetaVarEnv<'tree, D>,
    strictness: &MatchStrictness,
) -> bool {
    matches!(
        match_node_impl(pattern, candidate, env, strictness),
        MatchOneNode::MatchedBoth
    )
}

/// Compute the end byte offset of a match (for replacement range).
pub fn match_end<D: Doc>(
    pattern: &PatternNode,
    candidate: &Node<'_, D>,
    strictness: &MatchStrictness,
) -> Option<usize> {
    let mut env = MetaVarEnv::new();
    if match_pattern(pattern, candidate, &mut env, strictness) {
        Some(candidate.range().end)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_is_not_ellipsis() {
        let node = PatternNode::Terminal {
            text: "foo".into(),
            is_named: true,
            kind_id: 1,
        };
        assert!(!node.is_ellipsis());
    }

    #[test]
    fn ellipsis_detected() {
        let node = PatternNode::MetaVar {
            meta_var: MetaVariable::Ellipsis,
        };
        assert!(node.is_ellipsis());
    }

    #[test]
    fn multi_capture_is_ellipsis() {
        let node = PatternNode::MetaVar {
            meta_var: MetaVariable::MultiCapture("ARGS".into()),
        };
        assert!(node.is_ellipsis());
    }

    #[test]
    fn trivial_detection() {
        let named = PatternNode::Terminal { text: "x".into(), is_named: true, kind_id: 1 };
        let unnamed = PatternNode::Terminal { text: ",".into(), is_named: false, kind_id: 2 };
        assert!(!named.is_trivial());
        assert!(unnamed.is_trivial());
    }

    #[test]
    fn kinds_matching() {
        assert!(are_kinds_matching(5, 5));
        assert!(!are_kinds_matching(5, 6));
        assert!(are_kinds_matching(TS_ERROR_KIND, 42)); // ERROR matches anything
    }
}
