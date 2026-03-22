//! Rule compiler — SerializableRule → compiled Rule.

use axe_core::match_tree::MatchStrictness;
use thiserror::Error;

use crate::rule::{Rule, SerializableRule};

/// Errors during rule compilation.
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("pattern compilation failed: {0}")]
    Pattern(String),
    #[error("invalid regex: {0}")]
    Regex(#[from] regex::Error),
    #[error("unknown kind: {0}")]
    UnknownKind(String),
    #[error("rule has no positive matcher (pattern, kind, regex, or matches)")]
    NoPositiveMatcher,
}

/// Context for rule compilation — provides pattern compilation and kind resolution.
pub struct CompileContext<F, K>
where
    F: Fn(&str) -> Result<(axe_core::match_tree::PatternNode, Option<bit_set::BitSet>), String>,
    K: Fn(&str) -> Option<u16>,
{
    pub compile_pattern: F,
    pub resolve_kind: K,
}

/// Compile a serializable rule into a compiled Rule.
pub fn compile_rule<F, K>(
    rule: &SerializableRule,
    ctx: &CompileContext<F, K>,
) -> Result<Rule, CompileError>
where
    F: Fn(&str) -> Result<(axe_core::match_tree::PatternNode, Option<bit_set::BitSet>), String>,
    K: Fn(&str) -> Option<u16>,
{
    // Composite rules.
    if let Some(all) = &rule.all {
        let compiled: Result<Vec<Rule>, _> = all.iter()
            .map(|r| compile_rule(r, ctx))
            .collect();
        return Ok(Rule::All(compiled?));
    }
    if let Some(any) = &rule.any {
        let compiled: Result<Vec<Rule>, _> = any.iter()
            .map(|r| compile_rule(r, ctx))
            .collect();
        return Ok(Rule::Any(compiled?));
    }
    if let Some(not) = &rule.not {
        let inner = compile_rule(not, ctx)?;
        return Ok(Rule::Not(Box::new(inner)));
    }

    // Collect atomic + relational matchers.
    let mut matchers = Vec::new();

    // Atomic: pattern.
    if let Some(pattern_str) = &rule.pattern {
        let (node, kinds) = (ctx.compile_pattern)(pattern_str)
            .map_err(CompileError::Pattern)?;
        matchers.push(Rule::Pattern {
            node,
            strictness: MatchStrictness::Smart,
            kinds,
        });
    }

    // Atomic: kind.
    if let Some(kind_str) = &rule.kind {
        let kind_id = (ctx.resolve_kind)(kind_str)
            .ok_or_else(|| CompileError::UnknownKind(kind_str.clone()))?;
        matchers.push(Rule::Kind { kind_id });
    }

    // Atomic: regex.
    if let Some(regex_str) = &rule.regex {
        let pattern = regex::Regex::new(regex_str)?;
        matchers.push(Rule::Regex { pattern });
    }

    // Relational: inside.
    if let Some(inside) = &rule.inside {
        let inner = compile_rule(&inside.rule, ctx)?;
        matchers.push(Rule::Inside(Box::new(inner)));
    }

    // Relational: has.
    if let Some(has) = &rule.has {
        let inner = compile_rule(&has.rule, ctx)?;
        matchers.push(Rule::Has(Box::new(inner)));
    }

    if matchers.is_empty() {
        return Err(CompileError::NoPositiveMatcher);
    }

    if matchers.len() == 1 {
        Ok(matchers.pop().unwrap())
    } else {
        Ok(Rule::All(matchers))
    }
}
