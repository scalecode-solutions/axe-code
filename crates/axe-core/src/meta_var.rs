//! Meta-variable capture and constraint environment.

use ahash::AHashMap;

use crate::source::{Content, Doc};

// ---------------------------------------------------------------------------
// MetaVariable
// ---------------------------------------------------------------------------

/// A meta-variable parsed from a pattern string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaVariable {
    /// `$VAR` — captures a single named node.
    Capture(String, bool),
    /// `$$$VAR` — captures zero or more nodes (ellipsis with name).
    MultiCapture(String),
    /// `$$$` — unnamed ellipsis, matches zero or more nodes without capture.
    Ellipsis,
    /// `$_` — anonymous single-node capture (matches but doesn't bind).
    Anonymous(bool),
}

impl MetaVariable {
    pub fn parse(name: &str, is_named: bool) -> Self {
        if name == "_" || name == "_!" {
            return MetaVariable::Anonymous(is_named);
        }
        if let Some(multi_name) = name.strip_prefix("$$") {
            if multi_name.is_empty() {
                return MetaVariable::Ellipsis;
            }
            return MetaVariable::MultiCapture(multi_name.to_string());
        }
        MetaVariable::Capture(name.to_string(), is_named)
    }

    pub fn is_capture(&self) -> bool {
        matches!(self, MetaVariable::Capture(..) | MetaVariable::MultiCapture(_))
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            MetaVariable::Capture(n, _) | MetaVariable::MultiCapture(n) => Some(n),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// extract_meta_var — used by Language::extract_meta_var
// ---------------------------------------------------------------------------

fn is_valid_meta_var_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Extract a meta-variable from source text based on the expando character.
///
/// After pattern pre-processing, `$VAR` becomes `µVAR` (where µ is the expando).
/// This function recognizes:
/// - `µµµ` → `Ellipsis` (unnamed multi-node wildcard)
/// - `µµµNAME` → `MultiCapture("NAME")`
/// - `µ_` → `Anonymous`
/// - `µNAME` → `Capture("NAME")`
pub fn extract_meta_var(src: &str, expando: char) -> Option<MetaVariable> {
    let ellipsis: String = std::iter::repeat(expando).take(3).collect();

    // Check for ellipsis (µµµ or µµµNAME)
    if src == ellipsis {
        return Some(MetaVariable::Ellipsis);
    }
    if let Some(trimmed) = src.strip_prefix(&ellipsis) {
        if !trimmed.chars().all(is_valid_meta_var_char) {
            return None;
        }
        if trimmed.starts_with('_') {
            return Some(MetaVariable::Ellipsis);
        } else {
            return Some(MetaVariable::MultiCapture(trimmed.to_owned()));
        }
    }

    // Check for single meta-var (µNAME)
    let single: String = std::iter::once(expando).collect();
    let trimmed = src.strip_prefix(&single)?;
    if trimmed.is_empty() || !trimmed.chars().all(is_valid_meta_var_char) {
        return None;
    }
    if trimmed == "_" {
        return Some(MetaVariable::Anonymous(true));
    }
    let is_named = trimmed.chars().next().unwrap().is_uppercase();
    Some(MetaVariable::Capture(trimmed.to_owned(), is_named))
}

// ---------------------------------------------------------------------------
// MetaVarEnv
// ---------------------------------------------------------------------------

/// Environment tracking meta-variable captures during pattern matching.
/// Uses [`AHashMap`] for fast hashing (addresses ast-grep issue #449).
///
/// `Node<'tree, D>` is `Clone` (just copies inner handle + root ref), so
/// cloning an env is cheap.
#[derive(Clone)]
pub struct MetaVarEnv<'tree, D: Doc> {
    single: AHashMap<String, crate::node::Node<'tree, D>>,
    multi: AHashMap<String, Vec<crate::node::Node<'tree, D>>>,
    transformed: AHashMap<String, Vec<<D::Source as Content>::Underlying>>,
}

impl<'tree, D: Doc> MetaVarEnv<'tree, D> {
    pub fn new() -> Self {
        Self {
            single: AHashMap::new(),
            multi: AHashMap::new(),
            transformed: AHashMap::new(),
        }
    }

    pub fn insert_single(&mut self, name: &str, node: crate::node::Node<'tree, D>) -> bool {
        if let Some(existing) = self.single.get(name) {
            existing.text() == node.text()
        } else {
            self.single.insert(name.to_string(), node);
            true
        }
    }

    pub fn insert_multi(&mut self, name: String, nodes: Vec<crate::node::Node<'tree, D>>) {
        self.multi.insert(name, nodes);
    }

    pub fn get_match(&self, name: &str) -> Option<&crate::node::Node<'tree, D>> {
        self.single.get(name)
    }

    pub fn get_multiple_matches(&self, name: &str) -> &[crate::node::Node<'tree, D>] {
        self.multi.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn get_transformed(&self, name: &str) -> Option<&[<D::Source as Content>::Underlying]> {
        self.transformed.get(name).map(|v| v.as_slice())
    }

    pub fn insert_transformed(&mut self, name: String, value: Vec<<D::Source as Content>::Underlying>) {
        self.transformed.insert(name, value);
    }
}

impl<'tree, D: Doc> Default for MetaVarEnv<'tree, D> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_capture() {
        let mv = MetaVariable::parse("A", true);
        assert_eq!(mv, MetaVariable::Capture("A".into(), true));
        assert!(mv.is_capture());
        assert_eq!(mv.name(), Some("A"));
    }

    #[test]
    fn parse_multi_capture() {
        let mv = MetaVariable::parse("$$ARGS", true);
        assert_eq!(mv, MetaVariable::MultiCapture("ARGS".into()));
    }

    #[test]
    fn parse_ellipsis() {
        let mv = MetaVariable::parse("$$", true);
        assert_eq!(mv, MetaVariable::Ellipsis);
    }

    #[test]
    fn parse_anonymous() {
        let mv = MetaVariable::parse("_", true);
        assert_eq!(mv, MetaVariable::Anonymous(true));
    }
}
