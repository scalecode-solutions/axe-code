//! Rule configuration, deserialization, and combined scanning for axe.
//!
//! This crate handles:
//! - Rule deserialization from config files (using forma, not serde)
//! - Rule composition (atomic, relational, composite)
//! - Combined scan engine with kind pre-filtering
//! - Fix configuration and application
//! - Transform pipeline
//! - Suppression comments (`// axe-ignore`)

pub mod combined;
pub mod fixer;
pub mod rule;
pub mod rule_collection;
pub mod rule_config;
pub mod transform;

pub use combined::CombinedScan;
pub use rule::Rule;
pub use rule_config::{RuleConfig, Severity};
