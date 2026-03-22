//! Source document abstraction — encoding-agnostic content access.
//!
//! The [`Doc`] trait is the central abstraction in axe-core. It connects a
//! source document to its language, encoding, and node representation. The
//! [`Content`] trait abstracts over encodings (UTF-8, UTF-16, char vectors)
//! so the same matching algorithm works across CLI, NAPI, and WASM.

use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

// ---------------------------------------------------------------------------
// Edit
// ---------------------------------------------------------------------------

/// A text edit: replace `[start_byte..end_byte)` with `inserted_text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit<C: Content> {
    /// Byte offset of the edit start in the source encoding.
    pub start_byte: usize,
    /// Byte offset of the edit end (exclusive).
    pub end_byte: usize,
    /// Replacement content in the source encoding.
    pub inserted_text: Vec<C::Underlying>,
}

// ---------------------------------------------------------------------------
// Content trait — encoding abstraction
// ---------------------------------------------------------------------------

/// Abstracts over source-code encodings.
///
/// | Frontend | Underlying | Rationale |
/// |----------|-----------|-----------|
/// | CLI      | `u8`      | UTF-8, zero-copy from disk |
/// | NAPI     | `u16`     | UTF-16, matches V8 strings |
/// | WASM     | `char`    | Safe Unicode, no raw pointers |
pub trait Content: Clone + 'static {
    /// The atomic unit of this encoding.
    type Underlying: Clone + PartialEq + Eq + fmt::Debug;

    /// Extract a byte-range slice from the content.
    fn get_range(&self, range: Range<usize>) -> &[Self::Underlying];

    /// Decode a `&str` into this encoding.
    fn decode_str(src: &str) -> Cow<'_, [Self::Underlying]>;

    /// Encode content bytes back to a UTF-8 string.
    fn encode_bytes(bytes: &[Self::Underlying]) -> Cow<'_, str>;

    /// Calculate the character-level column for a given byte offset.
    ///
    /// `column` is the byte column reported by tree-sitter; `offset` is the
    /// byte offset of the line start. Returns the character column.
    fn get_char_column(&self, column: usize, offset: usize) -> usize;
}

// ---------------------------------------------------------------------------
// Content impl for Vec<u8> — UTF-8 (CLI)
// ---------------------------------------------------------------------------

impl Content for Vec<u8> {
    type Underlying = u8;

    #[inline]
    fn get_range(&self, range: Range<usize>) -> &[u8] {
        &self[range]
    }

    fn decode_str(src: &str) -> Cow<'_, [u8]> {
        Cow::Borrowed(src.as_bytes())
    }

    fn encode_bytes(bytes: &[u8]) -> Cow<'_, str> {
        String::from_utf8_lossy(bytes)
    }

    fn get_char_column(&self, column: usize, offset: usize) -> usize {
        // Use memchr-accelerated scan for the line, then count chars.
        let line_start = offset;
        let col_end = offset + column;
        if col_end > self.len() {
            return column;
        }
        let slice = &self[line_start..col_end];
        // Count UTF-8 leading bytes (bytes that are NOT continuation bytes).
        // A continuation byte has the pattern 10xxxxxx (0x80..0xBF).
        slice.iter().filter(|&&b| (b & 0xC0) != 0x80).count()
    }
}

// ---------------------------------------------------------------------------
// Content impl for Vec<u16> — UTF-16 (NAPI)
// ---------------------------------------------------------------------------

impl Content for Vec<u16> {
    type Underlying = u16;

    #[inline]
    fn get_range(&self, range: Range<usize>) -> &[u16] {
        // Range is in byte units; convert to u16 units.
        let start = range.start / 2;
        let end = range.end / 2;
        &self[start..end]
    }

    fn decode_str(src: &str) -> Cow<'_, [u16]> {
        Cow::Owned(src.encode_utf16().collect())
    }

    fn encode_bytes(bytes: &[u16]) -> Cow<'_, str> {
        Cow::Owned(String::from_utf16_lossy(bytes))
    }

    fn get_char_column(&self, column: usize, _offset: usize) -> usize {
        // UTF-16: column in u16 units is already ~char column for BMP.
        column / 2
    }
}

// ---------------------------------------------------------------------------
// SgNode trait — node abstraction
// ---------------------------------------------------------------------------

/// A borrowed AST node. Implementations wrap tree-sitter nodes (or other
/// parser backends) with a uniform interface.
pub trait SgNode<'r>: Clone + fmt::Debug {
    /// The node kind as a string (e.g., "function_declaration").
    fn kind(&self) -> &str;

    /// The node kind as a numeric ID (for fast comparison).
    fn kind_id(&self) -> u16;

    /// Whether this is a named node (as opposed to anonymous punctuation).
    fn is_named(&self) -> bool;

    /// Whether this is a leaf node (no children).
    fn is_leaf(&self) -> bool;

    /// The source text of this node.
    fn text(&self) -> &str;

    /// Byte range in the source content.
    fn byte_range(&self) -> Range<usize>;

    /// Start position (line, column) — zero-indexed.
    fn start_position(&self) -> (usize, usize);

    /// End position (line, column) — zero-indexed.
    fn end_position(&self) -> (usize, usize);

    /// Number of children.
    fn child_count(&self) -> usize;

    /// Get child by index.
    fn child(&self, index: usize) -> Option<Self>;

    /// Get child by field name.
    fn field_child(&self, field: &str) -> Option<Self>;

    /// Get all children.
    fn children(&self) -> Vec<Self>;

    /// Get named children only.
    fn named_children(&self) -> Vec<Self>;

    /// Parent node, if any.
    fn parent(&self) -> Option<Self>;

    /// Next sibling.
    fn next_sibling(&self) -> Option<Self>;

    /// Previous sibling.
    fn prev_sibling(&self) -> Option<Self>;

    /// Next named sibling.
    fn next_named_sibling(&self) -> Option<Self>;

    /// Previous named sibling.
    fn prev_named_sibling(&self) -> Option<Self>;

    /// Stable node ID (unique within a tree, not across re-parses).
    fn node_id(&self) -> usize;

    /// Whether this node is an ERROR node from the parser.
    fn is_error(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Doc trait — the central document abstraction
// ---------------------------------------------------------------------------

/// A parsed source document. Connects source content, language, and AST nodes.
///
/// `Doc` is the type parameter threaded through the entire matching engine.
/// Different frontends provide different implementations:
///
/// - CLI: `StrDoc<L>` — UTF-8, owned string
/// - NAPI: `JsDoc` — UTF-16, V8-compatible
/// - WASM: `WasmDoc` — char vector
pub trait Doc: Clone + 'static {
    /// The source content type (determines encoding).
    type Source: Content;

    /// The language type.
    type Lang: crate::language::Language;

    /// The AST node type (lifetime-parameterized).
    type Node<'r>: SgNode<'r>
    where
        Self: 'r;

    /// Get the source content.
    fn src(&self) -> &Self::Source;

    /// Get the language.
    fn lang(&self) -> &Self::Lang;

    /// Get the root AST node.
    fn root_node(&self) -> Self::Node<'_>;

    /// Re-parse with new source text (for incremental parsing).
    fn edit(&mut self, edit: &Edit<Self::Source>, new_src: Self::Source);
}

// ---------------------------------------------------------------------------
// IndentKind — TAB-aware indentation
// ---------------------------------------------------------------------------

/// Detected indentation style of a source document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentKind {
    /// Spaces, with a detected width (e.g., 2 or 4).
    Spaces(u8),
    /// Tab characters.
    Tabs,
    /// Could not detect — default to spaces.
    Unknown,
}

/// Detect the indentation style from source bytes.
pub fn detect_indent(src: &[u8]) -> IndentKind {
    let mut tab_lines = 0u32;
    let mut space_lines = 0u32;
    let mut space_widths = [0u32; 9]; // index = width, value = count

    for line in src.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        match line[0] {
            b'\t' => tab_lines += 1,
            b' ' => {
                let width = line.iter().take_while(|&&b| b == b' ').count();
                if width > 0 && width <= 8 {
                    space_widths[width] += 1;
                    space_lines += 1;
                }
            }
            _ => {}
        }
    }

    if tab_lines > space_lines {
        IndentKind::Tabs
    } else if space_lines > 0 {
        // Find the most common space width.
        let width = space_widths
            .iter()
            .enumerate()
            .skip(1)
            .max_by_key(|(_, count)| **count)
            .map(|(w, _)| w as u8)
            .unwrap_or(4);
        IndentKind::Spaces(width)
    } else {
        IndentKind::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_tabs() {
        let src = b"\tfoo\n\tbar\n\tbaz\n";
        assert_eq!(detect_indent(src), IndentKind::Tabs);
    }

    #[test]
    fn detect_2_spaces() {
        let src = b"  foo\n  bar\n  baz\n";
        assert_eq!(detect_indent(src), IndentKind::Spaces(2));
    }

    #[test]
    fn detect_4_spaces() {
        let src = b"    foo\n    bar\n    baz\n";
        assert_eq!(detect_indent(src), IndentKind::Spaces(4));
    }

    #[test]
    fn detect_unknown() {
        let src = b"foo\nbar\nbaz\n";
        assert_eq!(detect_indent(src), IndentKind::Unknown);
    }

    #[test]
    fn utf8_char_column() {
        let content: Vec<u8> = "let x = \"hello\"".as_bytes().to_vec();
        // All ASCII, so char column == byte column.
        assert_eq!(content.get_char_column(5, 0), 5);
    }
}
