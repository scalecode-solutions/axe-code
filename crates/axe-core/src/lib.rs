//! # axe-core
//!
//! Core pattern matching engine for **axe** — the AST eXpression Engine.
//!
//! This crate is encoding-agnostic and tree-sitter-agnostic. It defines the
//! fundamental traits (`Doc`, `Content`, `SgNode`, `Language`, `Matcher`) and
//! algorithms (pattern matching, meta-variable capture, replacer, traversal)
//! that all frontends (CLI, LSP, NAPI, PyO3, WASM) build on.
//!
//! ## Architecture
//!
//! - [`source`] — `Doc`, `Content`, `SgNode` traits (encoding abstraction)
//! - [`language`] — `Language` trait (parser abstraction)
//! - [`matcher`] — `Matcher` trait and built-in matchers (Pattern, Kind, Regex)
//! - [`node`] — `Node` and `Root` wrappers with safe lifetime management
//! - [`meta_var`] — Meta-variable capture and constraint environment
//! - [`match_tree`] — Core pattern-to-AST matching algorithm
//! - [`ops`] — Logical combinators (And, Or, Not, All, Any)
//! - [`replacer`] — Code transformation and template expansion

pub mod language;
pub mod match_tree;
pub mod matcher;
pub mod meta_var;
pub mod node;
pub mod ops;
pub mod replacer;
pub mod source;

pub use matcher::{Matcher, MatcherExt, Pattern};
pub use node::{Node, NodeMatch, Root};
pub use source::{Content, Doc};

/// Re-export for convenience. The `AstGrep` type is the primary entry point
/// for parsing source code and running queries.
pub type AstGrep<D> = Root<D>;
