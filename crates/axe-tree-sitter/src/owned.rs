//! Arc-based owned root for safe FFI boundaries.
//!
//! This replaces ast-grep's `PinnedNodeData` which used unsafe `'static`
//! lifetime transmutes. The `OwnedRoot` wraps a `Root` in an `Arc` so that
//! FFI consumers (NAPI, PyO3) can hold node references without risking
//! use-after-free.
//!
//! ## Design
//!
//! ```text
//! ┌──────────────────────────┐
//! │ Arc<RootInner<D>>        │  ← Reference counted, heap allocated
//! │  ├─ doc: D               │  ← Owns source + tree-sitter Tree
//! │  └─ (tree is inside doc) │
//! ├──────────────────────────┤
//! │ OwnedNode                │  ← Carries its own Arc clone
//! │  ├─ root: Arc<...>       │  ← Prevents Root from being freed
//! │  ├─ node_id: usize       │
//! │  ├─ start_byte: usize    │
//! │  └─ end_byte: usize      │
//! └──────────────────────────┘
//! ```
//!
//! **No unsafe code.** The Arc guarantees the tree stays alive.

use std::sync::Arc;

use axe_core::node::Root;
use axe_core::source::Doc;

/// An `Arc`-wrapped root for use across FFI boundaries.
///
/// Cloning is cheap (Arc reference count increment). The underlying document
/// and tree-sitter Tree are freed only when all references are dropped.
pub struct OwnedRoot<D: Doc> {
    inner: Arc<Root<D>>,
}

impl<D: Doc> OwnedRoot<D> {
    /// Wrap a root in an Arc for FFI use.
    pub fn new(root: Root<D>) -> Self {
        Self {
            inner: Arc::new(root),
        }
    }

    /// Get a reference to the inner root.
    pub fn root(&self) -> &Root<D> {
        &self.inner
    }

    /// Clone the Arc handle (cheap).
    pub fn clone_handle(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }

    /// Strong reference count.
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl<D: Doc> Clone for OwnedRoot<D> {
    fn clone(&self) -> Self {
        self.clone_handle()
    }
}

/// An owned node that carries its own Arc reference to the root.
///
/// This is the type exposed to NAPI/PyO3 consumers. It can outlive any
/// particular borrow scope because it holds the root alive via Arc.
pub struct OwnedNode<D: Doc> {
    root: OwnedRoot<D>,
    /// Stable node ID within the tree.
    pub node_id: usize,
    /// Byte range in the source.
    pub start_byte: usize,
    pub end_byte: usize,
    /// Node kind ID.
    pub kind_id: u16,
}

impl<D: Doc> OwnedNode<D> {
    /// Create from an owned root and node coordinates.
    pub fn new(
        root: OwnedRoot<D>,
        node_id: usize,
        start_byte: usize,
        end_byte: usize,
        kind_id: u16,
    ) -> Self {
        Self {
            root,
            node_id,
            start_byte,
            end_byte,
            kind_id,
        }
    }

    /// Access the root.
    pub fn root(&self) -> &OwnedRoot<D> {
        &self.root
    }
}

impl<D: Doc> Clone for OwnedNode<D> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            node_id: self.node_id,
            start_byte: self.start_byte,
            end_byte: self.end_byte,
            kind_id: self.kind_id,
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests will be added when we have a concrete Doc implementation
    // to construct OwnedRoot from.
}
