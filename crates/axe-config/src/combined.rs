//! Combined scan engine with kind-based dispatch.
//!
//! `CombinedScan` is the performance backbone: it builds a `kind_id -> [rule_index]`
//! mapping and does a single DFS traversal, dispatching to only the rules whose
//! `potential_kinds` include the current node's kind.
//!
//! This design is ported directly from ast-grep's `CombinedScan` because it's
//! excellent. The only change is using `AHashMap` for the dispatch table.

/// Placeholder — will be implemented when Rule compilation is complete.
pub struct CombinedScan {
    _private: (),
}
