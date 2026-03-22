//! Tree-sitter node wrapper implementing `SgNode`.

use axe_core::source::SgNode;
use std::ops::Range;

/// A tree-sitter node with a reference to its source text.
#[derive(Clone)]
pub struct TsNode<'r> {
    inner: tree_sitter::Node<'r>,
    src: &'r [u8],
}

impl<'r> TsNode<'r> {
    /// Wrap a tree-sitter node with its source text.
    pub fn new(inner: tree_sitter::Node<'r>, src: &'r [u8]) -> Self {
        Self { inner, src }
    }

    /// Access the raw tree-sitter node.
    pub fn raw(&self) -> &tree_sitter::Node<'r> {
        &self.inner
    }
}

impl<'r> std::fmt::Debug for TsNode<'r> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TsNode")
            .field("kind", &self.inner.kind())
            .field("range", &self.inner.byte_range())
            .finish()
    }
}

impl<'r> SgNode<'r> for TsNode<'r> {
    fn kind(&self) -> &str {
        self.inner.kind()
    }

    fn kind_id(&self) -> u16 {
        self.inner.kind_id()
    }

    fn is_named(&self) -> bool {
        self.inner.is_named()
    }

    fn is_leaf(&self) -> bool {
        self.inner.child_count() == 0
    }

    fn text(&self) -> &str {
        let range = self.inner.byte_range();
        std::str::from_utf8(&self.src[range]).unwrap_or("")
    }

    fn byte_range(&self) -> Range<usize> {
        self.inner.byte_range()
    }

    fn start_position(&self) -> (usize, usize) {
        let p = self.inner.start_position();
        (p.row, p.column)
    }

    fn end_position(&self) -> (usize, usize) {
        let p = self.inner.end_position();
        (p.row, p.column)
    }

    fn child_count(&self) -> usize {
        self.inner.child_count()
    }

    fn child(&self, index: usize) -> Option<Self> {
        self.inner
            .child(index as u32)
            .map(|n| TsNode::new(n, self.src))
    }

    fn field_child(&self, field: &str) -> Option<Self> {
        self.inner
            .child_by_field_name(field)
            .map(|n| TsNode::new(n, self.src))
    }

    fn children(&self) -> Vec<Self> {
        let mut cursor = self.inner.walk();
        self.inner
            .children(&mut cursor)
            .map(|n| TsNode::new(n, self.src))
            .collect()
    }

    fn named_children(&self) -> Vec<Self> {
        let mut cursor = self.inner.walk();
        self.inner
            .named_children(&mut cursor)
            .map(|n| TsNode::new(n, self.src))
            .collect()
    }

    fn parent(&self) -> Option<Self> {
        self.inner.parent().map(|n| TsNode::new(n, self.src))
    }

    fn next_sibling(&self) -> Option<Self> {
        self.inner
            .next_sibling()
            .map(|n| TsNode::new(n, self.src))
    }

    fn prev_sibling(&self) -> Option<Self> {
        self.inner
            .prev_sibling()
            .map(|n| TsNode::new(n, self.src))
    }

    fn next_named_sibling(&self) -> Option<Self> {
        self.inner
            .next_named_sibling()
            .map(|n| TsNode::new(n, self.src))
    }

    fn prev_named_sibling(&self) -> Option<Self> {
        self.inner
            .prev_named_sibling()
            .map(|n| TsNode::new(n, self.src))
    }

    fn node_id(&self) -> usize {
        self.inner.id()
    }

    fn is_error(&self) -> bool {
        self.inner.is_error()
    }
}
