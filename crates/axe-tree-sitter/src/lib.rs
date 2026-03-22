//! Tree-sitter integration for axe.
//!
//! This crate provides the concrete [`Doc`] and [`SgNode`] implementations
//! that connect axe-core's abstract matching engine to tree-sitter parsers.
//!
//! ## Key types
//!
//! - [`StrDoc`] — UTF-8 document backed by tree-sitter (for CLI use)
//! - [`TsNode`] — tree-sitter node wrapper implementing `SgNode`
//! - [`OwnedRoot`] — `Arc`-wrapped root for safe FFI (replaces ast-grep's `PinnedNodeData`)
//!
//! ## Memory safety
//!
//! Unlike ast-grep's `PinnedNodeData` which transmutes lifetimes to `'static`,
//! this crate uses `Arc<RootInner>` for the FFI case. The tree-sitter `Tree`
//! is heap-allocated and reference-counted — it cannot be freed while any
//! `OwnedNode` holds a reference.
//!
//! For the CLI hot path, normal borrowed `Node<'r, D>` lifetime tracking
//! applies — zero overhead, fully borrow-checked.

pub mod doc;
pub mod node;
pub mod owned;
pub mod pattern;
pub mod traversal;

pub use doc::StrDoc;
pub use node::TsNode;
pub use owned::OwnedRoot;
pub use pattern::TsPattern;
