//! Rule types — atomic, relational, and composite.

use forma_derive::{Deserialize, Serialize};

/// Serializable rule definition from config files.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    pub rule: SerializableRule,
    pub stop_by: Option<String>,
    pub field: Option<String>,
}

/// Compiled rule (placeholder).
pub enum Rule {
    Placeholder,
}
