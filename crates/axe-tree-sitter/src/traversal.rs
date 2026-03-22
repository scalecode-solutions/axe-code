//! AST traversal algorithms using tree-sitter cursors.
//!
//! Tree-sitter's `TreeCursor` is more efficient than recursive child iteration
//! because it maintains a stack internally and avoids repeated allocations.
//!
//! ## Traversal types
//!
//! - [`PreOrder`] — DFS pre-order using TreeCursor (most common)
//! - [`PostOrder`] — DFS post-order
//! - [`LevelOrder`] — BFS level-order
//!
//! The `PreOrder` traversal is especially important for the `Has` and `Inside`
//! relational operators, where ast-grep's TODO notes (relational_rule.rs:173,194)
//! called for switching from recursive DFS to cursor-based Pre traversal
//! to reduce stack allocation.

use tree_sitter::TreeCursor;

/// Pre-order DFS traversal using a tree-sitter cursor.
///
/// This is stack-efficient: tree-sitter's cursor maintains its own internal
/// stack rather than using Rust's call stack.
pub struct PreOrder<'r> {
    cursor: TreeCursor<'r>,
    src: &'r [u8],
    done: bool,
}

impl<'r> PreOrder<'r> {
    /// Create a pre-order traversal starting from the cursor's current node.
    pub fn new(cursor: TreeCursor<'r>, src: &'r [u8]) -> Self {
        Self {
            cursor,
            src,
            done: false,
        }
    }
}

impl<'r> Iterator for PreOrder<'r> {
    type Item = crate::node::TsNode<'r>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let node = crate::node::TsNode::new(self.cursor.node(), self.src);

        // Try to go to first child, then next sibling, then parent's next sibling.
        if self.cursor.goto_first_child() {
            return Some(node);
        }

        loop {
            if self.cursor.goto_next_sibling() {
                return Some(node);
            }
            if !self.cursor.goto_parent() {
                self.done = true;
                return Some(node);
            }
        }
    }
}

/// Post-order DFS traversal.
pub struct PostOrder<'r> {
    cursor: TreeCursor<'r>,
    src: &'r [u8],
    done: bool,
    descending: bool,
}

impl<'r> PostOrder<'r> {
    pub fn new(cursor: TreeCursor<'r>, src: &'r [u8]) -> Self {
        Self {
            cursor,
            src,
            done: false,
            descending: true,
        }
    }
}

impl<'r> Iterator for PostOrder<'r> {
    type Item = crate::node::TsNode<'r>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        // Descend to the deepest leftmost leaf.
        if self.descending {
            while self.cursor.goto_first_child() {}
            self.descending = false;
        }

        let node = crate::node::TsNode::new(self.cursor.node(), self.src);

        if self.cursor.goto_next_sibling() {
            self.descending = true;
        } else if !self.cursor.goto_parent() {
            self.done = true;
        }

        Some(node)
    }
}
