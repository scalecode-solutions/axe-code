//! AST node wrappers with safe lifetime management.
//!
//! [`Root`] owns the parsed document. [`Node`] borrows from it. [`NodeMatch`]
//! extends `Node` with a captured meta-variable environment.
//!
//! ## Memory safety model
//!
//! Unlike ast-grep's `PinnedNodeData` which transmutes lifetimes to `'static`,
//! axe uses straightforward Rust lifetimes:
//!
//! - For the CLI hot path, `Node<'r, D>` borrows from `Root<D>` — zero overhead.
//! - For FFI boundaries (NAPI, PyO3), `OwnedRoot` wraps `Root` in an `Arc`
//!   so nodes can carry a reference-counted handle. This is defined in
//!   `axe-tree-sitter`, not here.
//!
//! There is **no unsafe code** in this module.

use std::ops::Range;

use crate::meta_var::MetaVarEnv;
use crate::source::{Doc, SgNode};

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// A line/column position in source code (zero-indexed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Character column, accounting for encoding.
    pub fn column<D: Doc>(&self, node: &Node<'_, D>) -> usize {
        use crate::source::Content;
        let src = node.root.src();
        src.get_char_column(self.column, node.inner.byte_range().start)
    }
}

// ---------------------------------------------------------------------------
// Root — owns a parsed document
// ---------------------------------------------------------------------------

/// A parsed source document. Owns the `Doc` and provides the entry point
/// for creating borrowed `Node` handles.
pub struct Root<D: Doc> {
    doc: D,
}

impl<D: Doc> Root<D> {
    /// Create a new root from a parsed document.
    pub fn new(doc: D) -> Self {
        Self { doc }
    }

    /// Get the root AST node.
    pub fn root(&self) -> Node<'_, D> {
        Node {
            inner: self.doc.root_node(),
            root: self,
        }
    }

    /// Access the underlying document.
    pub fn doc(&self) -> &D {
        &self.doc
    }

    /// Access the language.
    pub fn lang(&self) -> &D::Lang {
        self.doc.lang()
    }

    /// Access the source content.
    pub fn src(&self) -> &D::Source {
        self.doc.src()
    }
}

impl<D: Doc> Clone for Root<D> {
    fn clone(&self) -> Self {
        Self {
            doc: self.doc.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Node — borrowed AST node
// ---------------------------------------------------------------------------

/// A borrowed AST node tied to a [`Root`]'s lifetime.
///
/// This is the primary node type used in the matching engine. It is cheap
/// to clone (just copies the inner node handle + root reference).
pub struct Node<'r, D: Doc> {
    pub(crate) inner: D::Node<'r>,
    pub(crate) root: &'r Root<D>,
}

impl<'r, D: Doc> Node<'r, D> {
    /// The source text of this node.
    pub fn text(&self) -> &str {
        self.inner.text()
    }

    /// The node kind as a string.
    pub fn kind(&self) -> &str {
        self.inner.kind()
    }

    /// The node kind as a numeric ID.
    pub fn kind_id(&self) -> u16 {
        self.inner.kind_id()
    }

    /// Whether this is a named node.
    pub fn is_named(&self) -> bool {
        self.inner.is_named()
    }

    /// Whether this is a leaf node.
    pub fn is_leaf(&self) -> bool {
        self.inner.is_leaf()
    }

    /// Byte range in the source.
    pub fn range(&self) -> Range<usize> {
        self.inner.byte_range()
    }

    /// Start position.
    pub fn start_pos(&self) -> Position {
        let (line, col) = self.inner.start_position();
        Position::new(line, col)
    }

    /// End position.
    pub fn end_pos(&self) -> Position {
        let (line, col) = self.inner.end_position();
        Position::new(line, col)
    }

    /// Number of children.
    pub fn child_count(&self) -> usize {
        self.inner.child_count()
    }

    /// Get child by index.
    pub fn child(&self, index: usize) -> Option<Node<'r, D>> {
        self.inner.child(index).map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Get all children.
    pub fn children(&self) -> impl Iterator<Item = Node<'r, D>> {
        self.inner.children().into_iter().map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Get named children only.
    pub fn named_children(&self) -> impl Iterator<Item = Node<'r, D>> {
        self.inner.named_children().into_iter().map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Get child by field name.
    pub fn field(&self, name: &str) -> Option<Node<'r, D>> {
        self.inner.field_child(name).map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Parent node.
    pub fn parent(&self) -> Option<Node<'r, D>> {
        self.inner.parent().map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Next sibling.
    pub fn next(&self) -> Option<Node<'r, D>> {
        self.inner.next_sibling().map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Previous sibling.
    pub fn prev(&self) -> Option<Node<'r, D>> {
        self.inner.prev_sibling().map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Next named sibling.
    pub fn next_named(&self) -> Option<Node<'r, D>> {
        self.inner.next_named_sibling().map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Previous named sibling.
    pub fn prev_named(&self) -> Option<Node<'r, D>> {
        self.inner.prev_named_sibling().map(|n| Node {
            inner: n,
            root: self.root,
        })
    }

    /// Iterate all ancestors (parent, grandparent, ...).
    pub fn ancestors(&self) -> Ancestors<'r, D> {
        Ancestors {
            current: self.parent(),
        }
    }

    /// DFS pre-order traversal of this subtree.
    pub fn dfs(&self) -> Dfs<'r, D> {
        Dfs {
            stack: vec![self.clone()],
        }
    }

    /// Whether this is an error node.
    pub fn is_error(&self) -> bool {
        self.inner.is_error()
    }

    /// Stable node ID.
    pub fn node_id(&self) -> usize {
        self.inner.node_id()
    }

    /// Access the language.
    pub fn lang(&self) -> &D::Lang {
        self.root.lang()
    }

    /// Access the root.
    pub fn get_root(&self) -> &'r Root<D> {
        self.root
    }

    /// Construct a Node from raw parts. Used by axe-tree-sitter.
    pub fn from_parts(inner: D::Node<'r>, root: &'r Root<D>) -> Self {
        Self { inner, root }
    }
}

// ---------------------------------------------------------------------------
// Pattern matching methods on Node
// ---------------------------------------------------------------------------

use crate::match_tree::{PatternNode, MatchStrictness, match_pattern};

impl<'r, D: Doc> Node<'r, D> {
    /// Find the first descendant (or self) matching a pattern.
    pub fn find_by_pattern(
        &self,
        pattern: &PatternNode,
        strictness: &MatchStrictness,
    ) -> Option<NodeMatch<'r, D>> {
        for node in self.dfs() {
            let mut env = MetaVarEnv::new();
            if match_pattern(pattern, &node, &mut env, strictness) {
                return Some(NodeMatch::new(node, env));
            }
        }
        None
    }

    /// Find all descendants (and self) matching a pattern.
    pub fn find_all_by_pattern(
        &self,
        pattern: &PatternNode,
        strictness: &MatchStrictness,
    ) -> Vec<NodeMatch<'r, D>> {
        let mut results = Vec::new();
        for node in self.dfs() {
            let mut env = MetaVarEnv::new();
            if match_pattern(pattern, &node, &mut env, strictness) {
                results.push(NodeMatch::new(node, env));
            }
        }
        results
    }

    /// Check if this node matches a pattern.
    pub fn matches_pattern(
        &self,
        pattern: &PatternNode,
        strictness: &MatchStrictness,
    ) -> bool {
        let mut env = MetaVarEnv::new();
        match_pattern(pattern, self, &mut env, strictness)
    }

    /// Check if any ancestor matches a pattern (`inside` relational rule).
    pub fn inside_pattern(
        &self,
        pattern: &PatternNode,
        strictness: &MatchStrictness,
    ) -> bool {
        for ancestor in self.ancestors() {
            let mut env = MetaVarEnv::new();
            if match_pattern(pattern, &ancestor, &mut env, strictness) {
                return true;
            }
        }
        false
    }

    /// Check if any descendant matches a pattern (`has` relational rule).
    pub fn has_pattern(
        &self,
        pattern: &PatternNode,
        strictness: &MatchStrictness,
    ) -> bool {
        // Skip self — `has` checks descendants.
        for node in self.dfs().skip(1) {
            let mut env = MetaVarEnv::new();
            if match_pattern(pattern, &node, &mut env, strictness) {
                return true;
            }
        }
        false
    }
}

impl<'r, D: Doc> Clone for Node<'r, D> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            root: self.root,
        }
    }
}

impl<'r, D: Doc> std::fmt::Debug for Node<'r, D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("kind", &self.kind())
            .field("text", &self.text())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Ancestors iterator
// ---------------------------------------------------------------------------

/// Iterator over ancestor nodes (parent, grandparent, ...).
pub struct Ancestors<'r, D: Doc> {
    current: Option<Node<'r, D>>,
}

impl<'r, D: Doc> Iterator for Ancestors<'r, D> {
    type Item = Node<'r, D>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.current.take()?;
        self.current = node.parent();
        Some(node)
    }
}

// ---------------------------------------------------------------------------
// DFS pre-order iterator
// ---------------------------------------------------------------------------

/// DFS pre-order traversal of a subtree.
pub struct Dfs<'r, D: Doc> {
    stack: Vec<Node<'r, D>>,
}

impl<'r, D: Doc> Iterator for Dfs<'r, D> {
    type Item = Node<'r, D>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        // Push children in reverse order so leftmost is popped first.
        let children: Vec<_> = node.children().collect();
        for child in children.into_iter().rev() {
            self.stack.push(child);
        }
        Some(node)
    }
}

// ---------------------------------------------------------------------------
// NodeMatch — node + captured environment
// ---------------------------------------------------------------------------

/// A matched node together with its meta-variable captures.
#[derive(Clone)]
pub struct NodeMatch<'r, D: Doc> {
    node: Node<'r, D>,
    env: MetaVarEnv<'r, D>,
}

impl<'r, D: Doc> NodeMatch<'r, D> {
    /// Create from a node and environment.
    pub fn new(node: Node<'r, D>, env: MetaVarEnv<'r, D>) -> Self {
        Self { node, env }
    }

    /// The matched node.
    pub fn node(&self) -> &Node<'r, D> {
        &self.node
    }

    /// The captured environment.
    pub fn env(&self) -> &MetaVarEnv<'r, D> {
        &self.env
    }

    /// Mutable access to the environment.
    pub fn env_mut(&mut self) -> &mut MetaVarEnv<'r, D> {
        &mut self.env
    }

    /// Consume into parts.
    pub fn into_parts(self) -> (Node<'r, D>, MetaVarEnv<'r, D>) {
        (self.node, self.env)
    }
}

impl<'r, D: Doc> std::ops::Deref for NodeMatch<'r, D> {
    type Target = Node<'r, D>;

    fn deref(&self) -> &Self::Target {
        &self.node
    }
}

impl<'r, D: Doc> From<Node<'r, D>> for NodeMatch<'r, D> {
    fn from(node: Node<'r, D>) -> Self {
        Self {
            node,
            env: MetaVarEnv::new(),
        }
    }
}
