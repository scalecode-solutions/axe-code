//! Rule configuration, deserialization, and combined scanning for axe.

pub mod combined;
pub mod compiler;
pub mod fixer;
pub mod rule;
pub mod rule_collection;
pub mod rule_config;
pub mod transform;

pub use combined::{CombinedScan, ScanHit};
pub use compiler::{compile_rule, CompileContext, CompileError};
pub use rule::Rule;
pub use rule_config::{ProjectConfig, RuleConfig, RuleTest, Severity};
