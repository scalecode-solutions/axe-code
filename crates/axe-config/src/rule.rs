//! Rule types — serializable and compiled.

use forma_derive::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Serializable rules (from config files)
// ---------------------------------------------------------------------------

/// Serializable rule definition from config files.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[forma(default)]
pub struct SerializableRule {
    pub pattern: Option<String>,
    pub kind: Option<String>,
    pub regex: Option<String>,
    pub inside: Option<Box<Relation>>,
    pub has: Option<Box<Relation>>,
    pub precedes: Option<Box<Relation>>,
    pub follows: Option<Box<Relation>>,
    pub all: Option<Vec<SerializableRule>>,
    pub any: Option<Vec<SerializableRule>>,
    pub not: Option<Box<SerializableRule>>,
    pub matches: Option<String>,
}

/// A relational rule with stop condition.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[forma(default)]
pub struct Relation {
    pub rule: SerializableRule,
    pub stop_by: Option<String>,
    pub field: Option<String>,
}

// ---------------------------------------------------------------------------
// Compiled rules (ready for matching)
// ---------------------------------------------------------------------------

use axe_core::match_tree::{PatternNode, MatchStrictness, match_pattern};
use axe_core::meta_var::MetaVarEnv;
use axe_core::node::{Node, NodeMatch};
use axe_core::source::Doc;
use bit_set::BitSet;

/// A compiled rule — ready to match against AST nodes.
///
/// Unlike ast-grep's Matcher trait (which is not dyn-compatible due to generic
/// methods), CompiledRule is a concrete enum. Each variant carries its own
/// matching logic and potential_kinds.
#[derive(Clone)]
pub enum Rule {
    /// Match by structural pattern.
    Pattern {
        node: PatternNode,
        strictness: MatchStrictness,
        kinds: Option<BitSet>,
    },
    /// Match by node kind string.
    Kind { kind_id: u16 },
    /// Match by regex on node text.
    Regex { pattern: regex::Regex },
    /// All sub-rules must match.
    All(Vec<Rule>),
    /// Any sub-rule must match.
    Any(Vec<Rule>),
    /// Sub-rule must NOT match.
    Not(Box<Rule>),
    /// Node must be inside an ancestor matching sub-rule.
    Inside(Box<Rule>),
    /// Node must have a descendant matching sub-rule.
    Has(Box<Rule>),
}

impl Rule {
    /// The set of node kind IDs this rule could match.
    /// `None` means any kind (unconstrained).
    pub fn potential_kinds(&self) -> Option<BitSet> {
        match self {
            Rule::Pattern { kinds, .. } => kinds.clone(),
            Rule::Kind { kind_id } => {
                let mut s = BitSet::new();
                s.insert(*kind_id as usize);
                Some(s)
            }
            Rule::Regex { .. } => None,
            Rule::All(rules) => {
                rules.iter().fold(None, |acc, r| {
                    let k = r.potential_kinds();
                    match (acc, k) {
                        (None, None) => None,
                        (Some(s), None) | (None, Some(s)) => Some(s),
                        (Some(mut a), Some(b)) => { a.intersect_with(&b); Some(a) }
                    }
                })
            }
            Rule::Any(rules) => {
                rules.iter().fold(Some(BitSet::new()), |acc, r| {
                    let k = r.potential_kinds();
                    match (acc, k) {
                        (None, _) | (_, None) => None,
                        (Some(mut a), Some(b)) => { a.union_with(&b); Some(a) }
                    }
                })
            }
            Rule::Not(_) => None,
            Rule::Inside(_) => None,
            Rule::Has(_) => None,
        }
    }

    /// Try to match this rule against a node, returning the matched node
    /// with captured meta-variables on success.
    pub fn match_node<'tree, D: Doc>(
        &self,
        node: Node<'tree, D>,
    ) -> Option<NodeMatch<'tree, D>> {
        let mut env = MetaVarEnv::new();
        if self.matches_with_env(&node, &mut env) {
            Some(NodeMatch::new(node, env))
        } else {
            None
        }
    }

    /// Check if this rule matches a node, populating the environment.
    fn matches_with_env<'tree, D: Doc>(
        &self,
        node: &Node<'tree, D>,
        env: &mut MetaVarEnv<'tree, D>,
    ) -> bool {
        match self {
            Rule::Pattern { node: pattern, strictness, .. } => {
                match_pattern(pattern, node, env, strictness)
            }
            Rule::Kind { kind_id } => {
                node.kind_id() == *kind_id
            }
            Rule::Regex { pattern } => {
                pattern.is_match(node.text())
            }
            Rule::All(rules) => {
                rules.iter().all(|r| r.matches_with_env(node, env))
            }
            Rule::Any(rules) => {
                rules.iter().any(|r| {
                    let mut trial = env.clone();
                    if r.matches_with_env(node, &mut trial) {
                        *env = trial;
                        true
                    } else {
                        false
                    }
                })
            }
            Rule::Not(inner) => {
                let mut throwaway = MetaVarEnv::new();
                !inner.matches_with_env(node, &mut throwaway)
            }
            Rule::Inside(inner) => {
                for ancestor in node.ancestors() {
                    let mut trial = MetaVarEnv::new();
                    if inner.matches_with_env(&ancestor, &mut trial) {
                        return true;
                    }
                }
                false
            }
            Rule::Has(inner) => {
                // Skip self, search descendants.
                for descendant in node.dfs().skip(1) {
                    let mut trial = MetaVarEnv::new();
                    if inner.matches_with_env(&descendant, &mut trial) {
                        return true;
                    }
                }
                false
            }
        }
    }
}

impl std::fmt::Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rule::Pattern { .. } => write!(f, "Rule::Pattern"),
            Rule::Kind { kind_id } => write!(f, "Rule::Kind({kind_id})"),
            Rule::Regex { pattern } => write!(f, "Rule::Regex({})", pattern.as_str()),
            Rule::All(r) => write!(f, "Rule::All({})", r.len()),
            Rule::Any(r) => write!(f, "Rule::Any({})", r.len()),
            Rule::Not(_) => write!(f, "Rule::Not(..)"),
            Rule::Inside(_) => write!(f, "Rule::Inside(..)"),
            Rule::Has(_) => write!(f, "Rule::Has(..)"),
        }
    }
}
