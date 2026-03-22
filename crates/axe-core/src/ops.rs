//! Logical combinators for composing matchers.

use bit_set::BitSet;

use crate::matcher::Matcher;
use crate::meta_var::MetaVarEnv;
use crate::node::Node;
use crate::source::Doc;

// ---------------------------------------------------------------------------
// And
// ---------------------------------------------------------------------------

/// Both matchers must match. Kind set is the intersection.
pub struct And<M1: Matcher, M2: Matcher> {
    first: M1,
    second: M2,
}

impl<M1: Matcher, M2: Matcher> And<M1, M2> {
    pub fn new(first: M1, second: M2) -> Self {
        Self { first, second }
    }
}

impl<M1: Matcher, M2: Matcher> Matcher for And<M1, M2> {
    fn match_node_with_env<'tree, D: Doc>(
        &self,
        node: Node<'tree, D>,
        env: &mut MetaVarEnv<'tree, D>,
    ) -> Option<Node<'tree, D>> {
        let node = self.first.match_node_with_env(node, env)?;
        self.second.match_node_with_env(node, env)
    }

    fn potential_kinds(&self) -> Option<BitSet> {
        intersect_kinds(self.first.potential_kinds(), self.second.potential_kinds())
    }
}

// ---------------------------------------------------------------------------
// Or
// ---------------------------------------------------------------------------

/// First matcher that succeeds wins. Kind set is the union.
pub struct Or<M1: Matcher, M2: Matcher> {
    first: M1,
    second: M2,
}

impl<M1: Matcher, M2: Matcher> Or<M1, M2> {
    pub fn new(first: M1, second: M2) -> Self {
        Self { first, second }
    }
}

impl<M1: Matcher, M2: Matcher> Matcher for Or<M1, M2> {
    fn match_node_with_env<'tree, D: Doc>(
        &self,
        node: Node<'tree, D>,
        env: &mut MetaVarEnv<'tree, D>,
    ) -> Option<Node<'tree, D>> {
        // Try first with a snapshot; on failure, try second.
        let snapshot = env.clone();
        if let Some(n) = self.first.match_node_with_env(node.clone(), env) {
            return Some(n);
        }
        *env = snapshot;
        self.second.match_node_with_env(node, env)
    }

    fn potential_kinds(&self) -> Option<BitSet> {
        union_kinds(self.first.potential_kinds(), self.second.potential_kinds())
    }
}

// ---------------------------------------------------------------------------
// Not
// ---------------------------------------------------------------------------

/// Inverts a matcher: succeeds when the inner matcher fails.
pub struct Not<M: Matcher> {
    inner: M,
}

impl<M: Matcher> Not<M> {
    pub fn new(inner: M) -> Self {
        Self { inner }
    }
}

impl<M: Matcher> Matcher for Not<M> {
    fn match_node_with_env<'tree, D: Doc>(
        &self,
        node: Node<'tree, D>,
        _env: &mut MetaVarEnv<'tree, D>,
    ) -> Option<Node<'tree, D>> {
        let mut throwaway = MetaVarEnv::new();
        if self.inner.match_node_with_env(node.clone(), &mut throwaway).is_some() {
            None
        } else {
            Some(node)
        }
    }

    fn potential_kinds(&self) -> Option<BitSet> {
        None
    }
}

// ---------------------------------------------------------------------------
// Kind set utilities
// ---------------------------------------------------------------------------

fn intersect_kinds(a: Option<BitSet>, b: Option<BitSet>) -> Option<BitSet> {
    match (a, b) {
        (None, None) => None,
        (Some(s), None) | (None, Some(s)) => Some(s),
        (Some(mut a), Some(b)) => {
            a.intersect_with(&b);
            Some(a)
        }
    }
}

fn union_kinds(a: Option<BitSet>, b: Option<BitSet>) -> Option<BitSet> {
    match (a, b) {
        (None, _) | (_, None) => None,
        (Some(mut a), Some(b)) => {
            a.union_with(&b);
            Some(a)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersect_none_none() {
        assert!(intersect_kinds(None, None).is_none());
    }

    #[test]
    fn intersect_some_none() {
        let mut s = BitSet::new();
        s.insert(5);
        let result = intersect_kinds(Some(s), None).unwrap();
        assert!(result.contains(5));
    }

    #[test]
    fn intersect_some_some() {
        let mut a = BitSet::new();
        a.insert(1);
        a.insert(2);
        let mut b = BitSet::new();
        b.insert(2);
        b.insert(3);
        let result = intersect_kinds(Some(a), Some(b)).unwrap();
        assert!(!result.contains(1));
        assert!(result.contains(2));
        assert!(!result.contains(3));
    }

    #[test]
    fn union_none_any() {
        let s = BitSet::new();
        assert!(union_kinds(None, Some(s)).is_none());
    }

    #[test]
    fn union_some_some() {
        let mut a = BitSet::new();
        a.insert(1);
        let mut b = BitSet::new();
        b.insert(2);
        let result = union_kinds(Some(a), Some(b)).unwrap();
        assert!(result.contains(1));
        assert!(result.contains(2));
    }
}
