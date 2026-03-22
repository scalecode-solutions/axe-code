//! Language abstraction trait.
//!
//! The [`Language`] trait defines the interface between axe's matching engine
//! and a specific programming language's parser. It handles meta-variable
//! character conventions, pattern pre-processing, and parser-specific details.

use std::borrow::Cow;

/// A programming language parser abstraction.
///
/// Implementations wrap tree-sitter grammars (or other parsers) and provide
/// the language-specific hooks that the matching engine needs.
pub trait Language: Clone + 'static {
    /// Pre-process a pattern string before parsing.
    ///
    /// This is where language-specific meta-variable substitution happens.
    /// For example, in PHP, `$VAR` is a valid PHP variable, so patterns use
    /// `#VAR` and this method converts `#VAR` -> `µVAR` for the parser.
    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str>;

    /// The character users write for meta-variables in patterns.
    ///
    /// Almost always `'$'`. PHP uses `'#'` because `$` is PHP syntax.
    fn meta_var_char(&self) -> char {
        '$'
    }

    /// The internal character used as a meta-variable placeholder during parsing.
    ///
    /// Must be a character that the language's parser accepts as an identifier
    /// start. Defaults to `'µ'` (micro sign), which works for most grammars.
    fn expando_char(&self) -> char {
        'µ'
    }

    /// Convert a node kind string to its numeric ID.
    fn kind_to_id(&self, kind: &str) -> Option<u16>;

    /// Convert a numeric kind ID back to its string name.
    fn id_to_kind(&self, id: u16) -> &str;

    /// Convert a field name to its numeric ID.
    fn field_to_id(&self, field: &str) -> Option<u16>;

    /// The total number of node kinds in this language's grammar.
    fn kind_count(&self) -> usize;

    /// Extract a meta-variable from a node's text, if it is one.
    ///
    /// After pre-processing, patterns contain `µVAR` where the user wrote `$VAR`.
    /// This method recognizes those placeholders and returns the appropriate
    /// `MetaVariable` variant.
    fn extract_meta_var(&self, source: &str) -> Option<crate::meta_var::MetaVariable> {
        crate::meta_var::extract_meta_var(source, self.expando_char())
    }
}
