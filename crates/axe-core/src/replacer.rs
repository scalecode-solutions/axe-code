//! Code transformation and template expansion.
//!
//! The replacer system takes a matched node with captured meta-variables and
//! produces replacement text. It handles:
//!
//! - Template expansion: `$VAR` substitution in replacement strings
//! - Indentation preservation: replacement inherits the matched node's indent
//! - TAB-aware indentation (fixes ast-grep's spaces-only limitation)
//! - Multi-line replacement alignment

use crate::source::{IndentKind, detect_indent};

// ---------------------------------------------------------------------------
// Indentation helpers
// ---------------------------------------------------------------------------

/// Extract the leading whitespace of a line at the given byte offset.
pub fn leading_whitespace(src: &[u8], line_start: usize) -> &[u8] {
    let rest = &src[line_start..];
    let end = rest
        .iter()
        .position(|&b| b != b' ' && b != b'\t')
        .unwrap_or(rest.len());
    &rest[..end]
}

/// Deindent a block of text by removing `prefix_len` characters of leading
/// whitespace from each line.
pub fn deindent(text: &str, indent: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        if let Some(stripped) = line.strip_prefix(indent) {
            result.push_str(stripped);
        } else {
            // Line has less indentation than expected — emit as-is.
            result.push_str(line);
        }
    }
    // Preserve trailing newline if present.
    if text.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Re-indent a block of text to match a target indentation.
pub fn reindent(text: &str, indent: &str) -> String {
    let mut result = String::with_capacity(text.len() + indent.len() * 10);
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.push('\n');
            if !line.is_empty() {
                result.push_str(indent);
            }
        }
        result.push_str(line);
    }
    if text.ends_with('\n') {
        result.push('\n');
    }
    result
}

// ---------------------------------------------------------------------------
// Template expansion
// ---------------------------------------------------------------------------

/// A segment of a replacement template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateSegment {
    /// Literal text — emit as-is.
    Literal(String),
    /// Meta-variable reference — substitute with captured value.
    MetaVar(String),
    /// Multi meta-variable reference — substitute with captured values.
    MultiMetaVar(String),
}

/// Parse a replacement template into segments.
///
/// Template syntax: `$VAR` for single captures, `$$$VAR` for multi captures.
/// Use `$$` to escape a literal `$`.
pub fn parse_template(template: &str, meta_var_char: char) -> Vec<TemplateSegment> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == meta_var_char {
            // Check for multi-capture ($$$VAR)
            if chars.peek() == Some(&meta_var_char) {
                chars.next();
                if chars.peek() == Some(&meta_var_char) {
                    chars.next();
                    // $$$VAR — multi-capture
                    let name = take_identifier(&mut chars);
                    if !name.is_empty() {
                        if !literal.is_empty() {
                            segments.push(TemplateSegment::Literal(std::mem::take(&mut literal)));
                        }
                        segments.push(TemplateSegment::MultiMetaVar(name));
                    } else {
                        // Bare $$$ — literal
                        literal.push(meta_var_char);
                        literal.push(meta_var_char);
                        literal.push(meta_var_char);
                    }
                } else {
                    // $$ — escaped dollar sign
                    literal.push(meta_var_char);
                }
            } else {
                // $VAR — single capture
                let name = take_identifier(&mut chars);
                if !name.is_empty() {
                    if !literal.is_empty() {
                        segments.push(TemplateSegment::Literal(std::mem::take(&mut literal)));
                    }
                    segments.push(TemplateSegment::MetaVar(name));
                } else {
                    literal.push(meta_var_char);
                }
            }
        } else {
            literal.push(c);
        }
    }

    if !literal.is_empty() {
        segments.push(TemplateSegment::Literal(literal));
    }

    segments
}

/// Consume an identifier (alphanumeric + underscore) from the iterator.
fn take_identifier(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut name = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_alphanumeric() || c == '_' {
            name.push(c);
            chars.next();
        } else {
            break;
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_template() {
        let segs = parse_template("logger.info($A)", '$');
        assert_eq!(
            segs,
            vec![
                TemplateSegment::Literal("logger.info(".into()),
                TemplateSegment::MetaVar("A".into()),
                TemplateSegment::Literal(")".into()),
            ]
        );
    }

    #[test]
    fn parse_multi_var_template() {
        let segs = parse_template("fn($$$ARGS)", '$');
        assert_eq!(
            segs,
            vec![
                TemplateSegment::Literal("fn(".into()),
                TemplateSegment::MultiMetaVar("ARGS".into()),
                TemplateSegment::Literal(")".into()),
            ]
        );
    }

    #[test]
    fn parse_escaped_dollar() {
        let segs = parse_template("cost is $$5", '$');
        assert_eq!(
            segs,
            vec![TemplateSegment::Literal("cost is $5".into())]
        );
    }

    #[test]
    fn deindent_basic() {
        let text = "    foo\n    bar\n    baz\n";
        assert_eq!(deindent(text, "    "), "foo\nbar\nbaz\n");
    }

    #[test]
    fn reindent_basic() {
        let text = "foo\nbar\nbaz";
        assert_eq!(reindent(text, "  "), "foo\n  bar\n  baz");
    }

    #[test]
    fn leading_ws_spaces() {
        let src = b"    hello world";
        assert_eq!(leading_whitespace(src, 0), b"    ");
    }

    #[test]
    fn leading_ws_tabs() {
        let src = b"\t\thello world";
        assert_eq!(leading_whitespace(src, 0), b"\t\t");
    }

    #[test]
    fn detect_indent_tabs() {
        assert_eq!(detect_indent(b"\tfoo\n\tbar\n"), IndentKind::Tabs);
    }

    #[test]
    fn detect_indent_spaces() {
        assert_eq!(detect_indent(b"  foo\n  bar\n"), IndentKind::Spaces(2));
    }
}
