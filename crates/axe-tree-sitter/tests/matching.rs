//! End-to-end pattern matching tests.
//!
//! Compile a pattern → parse source → traverse AST → run matching algorithm.

use axe_core::match_tree::{match_pattern, MatchStrictness};
use axe_core::meta_var::MetaVarEnv;
use axe_core::node::{Node, Root};
use axe_tree_sitter::doc::StrDoc;
use axe_tree_sitter::pattern::TsPattern;
use axe_language::JavaScript;

type JsDoc = StrDoc<JavaScript>;

fn parse_js(src: &str) -> Root<JsDoc> {
    let doc = StrDoc::new(src, JavaScript, JavaScript::ts_language()).unwrap();
    Root::new(doc)
}

fn compile_js(pattern: &str) -> TsPattern {
    TsPattern::new(pattern, &JavaScript, JavaScript::ts_language()).unwrap()
}

/// DFS search: try to match `pattern` against any node in the tree.
fn find_match_in_tree<'r>(
    node: Node<'r, JsDoc>,
    pattern: &TsPattern,
) -> bool {
    let mut env = MetaVarEnv::<JsDoc>::new();
    if match_pattern(&pattern.node, &node, &mut env, &pattern.strictness) {
        return true;
    }
    for child in node.children() {
        if find_match_in_tree(child, pattern) {
            return true;
        }
    }
    false
}

/// DFS search that returns captured meta-variables on first match.
fn find_and_capture<'r>(
    node: Node<'r, JsDoc>,
    pattern: &TsPattern,
) -> Option<MetaVarEnv<'r, JsDoc>> {
    let mut env = MetaVarEnv::<JsDoc>::new();
    if match_pattern(&pattern.node, &node, &mut env, &pattern.strictness) {
        return Some(env);
    }
    for child in node.children() {
        if let Some(env) = find_and_capture(child, pattern) {
            return Some(env);
        }
    }
    None
}

// -----------------------------------------------------------------------
// Basic matching
// -----------------------------------------------------------------------

#[test]
fn match_console_log() {
    let root = parse_js("function foo() { console.log('hello'); }");
    let pat = compile_js("console.log($A)");
    assert!(find_match_in_tree(root.root(), &pat));
}

#[test]
fn no_match_when_absent() {
    let root = parse_js("function foo() { return 42; }");
    let pat = compile_js("console.log($A)");
    assert!(!find_match_in_tree(root.root(), &pat));
}

#[test]
fn match_with_different_args() {
    let pat = compile_js("console.log($A)");
    for src in &[
        "console.log(42)",
        "console.log('hello')",
        "console.log(x + y)",
        "console.log(fn())",
    ] {
        let root = parse_js(src);
        assert!(find_match_in_tree(root.root(), &pat), "should match: {src}");
    }
}

#[test]
fn match_variable_declaration() {
    let root = parse_js("const x = 42;");
    let pat = compile_js("const $A = $B");
    assert!(find_match_in_tree(root.root(), &pat));
}

#[test]
fn no_match_let_vs_const() {
    let root = parse_js("let x = 42;");
    let pat = compile_js("const $A = $B");
    assert!(!find_match_in_tree(root.root(), &pat));
}

// -----------------------------------------------------------------------
// Meta-variable capture
// -----------------------------------------------------------------------

#[test]
fn capture_single_var() {
    let root = parse_js("console.log('hello')");
    let pat = compile_js("console.log($A)");
    let env = find_and_capture(root.root(), &pat).expect("should match");
    let captured = env.get_match("A").expect("$A should be captured");
    assert_eq!(captured.text(), "'hello'");
}

#[test]
fn capture_function_name() {
    let root = parse_js("function greet() { return 1; }");
    let pat = compile_js("function $NAME() { $$$BODY }");
    let env = find_and_capture(root.root(), &pat).expect("should match");
    let name = env.get_match("NAME").expect("$NAME should be captured");
    assert_eq!(name.text(), "greet");
}

// -----------------------------------------------------------------------
// Multiple languages
// -----------------------------------------------------------------------

#[test]
fn match_rust_pattern() {
    use axe_language::Rust;
    let doc = StrDoc::new("fn main() { println!(\"hi\"); }", Rust, Rust::ts_language()).unwrap();
    let root = Root::new(doc);
    let pat = TsPattern::new("fn $NAME() { $$$BODY }", &Rust, Rust::ts_language()).unwrap();

    fn find<'r>(node: Node<'r, StrDoc<Rust>>, pat: &TsPattern) -> bool {
        let mut env = MetaVarEnv::new();
        if match_pattern(&pat.node, &node, &mut env, &pat.strictness) {
            return true;
        }
        for child in node.children() {
            if find(child, pat) {
                return true;
            }
        }
        false
    }
    assert!(find(root.root(), &pat));
}

#[test]
fn match_python_pattern() {
    use axe_language::Python;
    let doc = StrDoc::new("print('hello')", Python, Python::ts_language()).unwrap();
    let root = Root::new(doc);
    let pat = TsPattern::new("print($A)", &Python, Python::ts_language()).unwrap();

    fn find<'r>(node: Node<'r, StrDoc<Python>>, pat: &TsPattern) -> bool {
        let mut env = MetaVarEnv::new();
        if match_pattern(&pat.node, &node, &mut env, &pat.strictness) {
            return true;
        }
        for child in node.children() {
            if find(child, pat) {
                return true;
            }
        }
        false
    }
    assert!(find(root.root(), &pat));
}
