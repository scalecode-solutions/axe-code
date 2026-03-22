//! UTF-8 document backed by tree-sitter.

use axe_core::language::Language;
use axe_core::source::{Content, Doc, Edit};
use tree_sitter::{Parser, Tree};

use crate::node::TsNode;

/// A UTF-8 source document parsed by tree-sitter.
///
/// This is the primary `Doc` implementation for CLI use.
pub struct StrDoc<L: Language> {
    src: Vec<u8>,
    tree: Tree,
    lang: L,
}

impl<L: Language> StrDoc<L> {
    /// Parse a string with the given language.
    pub fn new(src: &str, lang: L, ts_language: tree_sitter::Language) -> Result<Self, ParseError> {
        let mut parser = Parser::new();
        parser
            .set_language(&ts_language)
            .map_err(|e| ParseError::Language(e.to_string()))?;
        let tree = parser
            .parse(src.as_bytes(), None)
            .ok_or(ParseError::Timeout)?;
        Ok(Self {
            src: src.as_bytes().to_vec(),
            tree,
            lang,
        })
    }

    /// Access the tree-sitter Tree.
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// Get source as a string slice.
    pub fn source_text(&self) -> &str {
        // Safe: we constructed from a &str, so the bytes are valid UTF-8.
        std::str::from_utf8(&self.src).unwrap_or("")
    }
}

impl<L: Language> Clone for StrDoc<L> {
    fn clone(&self) -> Self {
        Self {
            src: self.src.clone(),
            tree: self.tree.clone(),
            lang: self.lang.clone(),
        }
    }
}

impl<L: Language> Doc for StrDoc<L> {
    type Source = Vec<u8>;
    type Lang = L;
    type Node<'r> = TsNode<'r>;

    fn src(&self) -> &Vec<u8> {
        &self.src
    }

    fn lang(&self) -> &L {
        &self.lang
    }

    fn root_node(&self) -> TsNode<'_> {
        TsNode::new(self.tree.root_node(), &self.src)
    }

    fn edit(&mut self, _edit: &Edit<Vec<u8>>, new_src: Vec<u8>) {
        // TODO: implement incremental parsing via tree.edit() + parser.parse(new, Some(&old))
        // For now, full re-parse.
        let mut parser = Parser::new();
        // Note: we'd need the ts_language here. For now this is a placeholder.
        // The incremental parsing story will be completed when axe-lsp needs it.
        let _ = &new_src;
        self.src = new_src;
    }
}

/// Errors that can occur during tree-sitter parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("language error: {0}")]
    Language(String),
    #[error("parse timed out")]
    Timeout,
}
