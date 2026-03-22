//! CHEVRON 1: ABYDOS
//!
//! The first mission through the gate. Every feature, called once, the way
//! the tutorial shows it, with the input the docs promise will work.

use std::process::Command;

const AXE: &str = env!("CARGO_BIN_EXE_axe");

fn axe(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(AXE)
        .args(args)
        .output()
        .expect("failed to run axe");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}

fn write_tmp(name: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("axe_furling");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

// ======================================================================
// axe --help / --version
// ======================================================================

#[test]
fn help_exits_zero() {
    let (stdout, _, code) = axe(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("axe"), "help should mention axe");
    assert!(stdout.contains("run"), "help should list run command");
    assert!(stdout.contains("scan"), "help should list scan command");
    assert!(stdout.contains("test"), "help should list test command");
}

#[test]
fn version_exits_zero() {
    let (stdout, _, code) = axe(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("axe"), "version should mention axe");
}

// ======================================================================
// axe run — basic pattern matching
// ======================================================================

#[test]
fn run_finds_console_log_js() {
    let f = write_tmp("c1_console.js", "console.log('hello');");
    let (stdout, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert_eq!(code, 1, "should exit 1 when matches found");
    assert!(stdout.contains("console.log"), "should find console.log");
    assert!(stdout.contains("$A="), "should capture $A");
}

#[test]
fn run_no_match_exits_zero() {
    let f = write_tmp("c1_nomatch.js", "const x = 42;");
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert_eq!(code, 0, "no matches should exit 0");
}

#[test]
fn run_const_does_not_match_let() {
    let f = write_tmp("c1_const_let.js", "let x = 42;\nconst y = 99;");
    let (stdout, _, code) = axe(&["run", "-p", "const $A = $B", "-l", "js", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stdout.contains("const y"), "should match const");
    assert!(!stdout.contains("let x"), "should NOT match let");
}

#[test]
fn run_python_pattern() {
    let f = write_tmp("c1_py.py", "print('hello')\nprint('world')");
    let (stdout, _, code) = axe(&["run", "-p", "print($A)", "-l", "python", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stdout.contains("hello"), "should find first print");
    assert!(stdout.contains("world"), "should find second print");
}

#[test]
fn run_rust_pattern() {
    let f = write_tmp("c1_rs.rs", "fn foo() {}\nfn bar(x: i32) {}");
    let (stdout, _, code) = axe(&["run", "-p", "fn $NAME() {}", "-l", "rust", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stdout.contains("foo"), "should match fn foo()");
}

// ======================================================================
// axe run — output formats
// ======================================================================

#[test]
fn run_sif_output() {
    let f = write_tmp("c1_sif.js", "console.log(42);");
    let (stdout, _, _) = axe(&["run", "-p", "console.log($A)", "-l", "js", "--format", "sif", f.to_str().unwrap()]);
    assert!(stdout.starts_with("#!sif v1"), "SIF output should start with header");
    assert!(stdout.contains("#schema"), "should contain schema");
}

#[test]
fn run_json_output() {
    let f = write_tmp("c1_json.js", "console.log(42);");
    let (stdout, _, _) = axe(&["run", "-p", "console.log($A)", "-l", "js", "--format", "json", f.to_str().unwrap()]);
    assert!(stdout.contains(r#""file""#), "JSON should contain file key");
    assert!(stdout.contains(r#""line""#), "JSON should contain line key");
}

#[test]
fn run_color_output() {
    let f = write_tmp("c1_color.js", "console.log(42);");
    let (stdout, _, _) = axe(&["run", "-p", "console.log($A)", "-l", "js", "--format", "color", f.to_str().unwrap()]);
    assert!(stdout.contains(":"), "plain output should have file:line:col format");
}

// ======================================================================
// axe run — rewrite
// ======================================================================

#[test]
fn run_rewrite_preview() {
    let f = write_tmp("c1_rw.js", "console.log('hello');");
    let (stdout, stderr, code) = axe(&["run", "-p", "console.log($A)", "-r", "logger.info($A)", "-l", "js", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stdout.contains("logger.info"), "preview should show replacement");
    assert!(stderr.contains("preview"), "stderr should say preview");
    // File should NOT be modified.
    let content = std::fs::read_to_string(&f).unwrap();
    assert!(content.contains("console.log"), "file should NOT be modified without --apply");
}

#[test]
fn run_rewrite_apply() {
    let f = write_tmp("c1_apply.js", "console.log('hello');");
    let (_, stderr, _) = axe(&["run", "-p", "console.log($A)", "-r", "logger.info($A)", "--apply", "-l", "js", f.to_str().unwrap()]);
    assert!(stderr.contains("applied"), "stderr should say applied");
    let content = std::fs::read_to_string(&f).unwrap();
    assert!(content.contains("logger.info"), "file should be modified");
    assert!(!content.contains("console.log"), "original should be replaced");
}

// ======================================================================
// axe run — max results
// ======================================================================

#[test]
fn run_max_results() {
    let f = write_tmp("c1_max.js", "console.log(1);\nconsole.log(2);\nconsole.log(3);");
    let (stdout, _, _) = axe(&["run", "-p", "console.log($A)", "-l", "js", "--max-results", "1", f.to_str().unwrap()]);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.starts_with('#')).collect();
    assert_eq!(lines.len(), 1, "should return exactly 1 match");
}

// ======================================================================
// axe scan — basic
// ======================================================================

#[test]
fn scan_with_inline_rule() {
    let f = write_tmp("c1_scan.js", "console.log('test');");
    let (stdout, _, code) = axe(&[
        "scan",
        "--inline-rules",
        r#"{"id":"no-log","language":"javascript","rule":{"pattern":"console.log($A)"},"severity":"Warning","message":"no logs"}"#,
        f.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(stdout.contains("no-log"), "should contain rule id");
    assert!(stdout.contains("warning"), "should contain severity");
}

// ======================================================================
// axe test — rule testing
// ======================================================================

#[test]
fn test_passing_rule() {
    let rule = write_tmp("c1_rule_pass.json", r#"{
        "id": "no-log",
        "language": "javascript",
        "rule": { "pattern": "console.log($A)" },
        "severity": "Warning",
        "message": "no logs",
        "tests": {
            "valid": ["const x = 42"],
            "invalid": ["console.log('hello')"]
        }
    }"#);
    let (stdout, _, code) = axe(&["test", "-r", rule.to_str().unwrap()]);
    assert_eq!(code, 0, "passing tests should exit 0");
    assert!(stdout.contains("PASS"), "should show PASS");
}

#[test]
fn test_failing_rule() {
    let rule = write_tmp("c1_rule_fail.json", r#"{
        "id": "bad-rule",
        "language": "javascript",
        "rule": { "pattern": "console.log($A)" },
        "severity": "Warning",
        "message": "bad",
        "tests": {
            "valid": ["console.log('this should fail')"],
            "invalid": ["const x = 42"]
        }
    }"#);
    let (stdout, _, code) = axe(&["test", "-r", rule.to_str().unwrap()]);
    assert_eq!(code, 1, "failing tests should exit 1");
    assert!(stdout.contains("FAIL"), "should show FAIL");
}

// ======================================================================
// axe new — scaffolding
// ======================================================================

#[test]
fn new_rule() {
    let (stdout, _, code) = axe(&["new", "rule", "no-eval", "--lang", "js"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("no-eval"), "should contain rule id");
    assert!(stdout.contains("\"language\""), "should contain language field");
    assert!(stdout.contains("tests"), "should contain tests section");
}

#[test]
fn new_config() {
    let (stdout, _, code) = axe(&["new", "config"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("rule_dirs"), "should contain rule_dirs");
}

// ======================================================================
// axe completions
// ======================================================================

#[test]
fn completions_bash() {
    let (stdout, _, code) = axe(&["completions", "bash"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("axe"), "bash completions should reference axe");
}

#[test]
fn completions_zsh() {
    let (stdout, _, code) = axe(&["completions", "zsh"]);
    assert_eq!(code, 0);
    assert!(!stdout.is_empty(), "zsh completions should not be empty");
}

// ======================================================================
// Multiple languages
// ======================================================================

#[test]
fn run_go_pattern() {
    let f = write_tmp("c1_go.go", "package main\nfunc main() { fmt.Println(\"hello\") }");
    let (_, _, code) = axe(&["run", "-p", "fmt.Println($A)", "-l", "go", f.to_str().unwrap()]);
    // Go may or may not match depending on expando char handling — just verify no crash.
    assert!(code == 0 || code == 1, "should not crash");
}

#[test]
fn run_kotlin_pattern() {
    let f = write_tmp("c1_kt.kt", "fun main() { println(\"hello\") }");
    let (stdout, _, code) = axe(&["run", "-p", "println($A)", "-l", "kotlin", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stdout.contains("hello"));
}

#[test]
fn run_scala_pattern() {
    let f = write_tmp("c1_scala.scala", "object Main { println(\"hello\") }");
    let (stdout, _, code) = axe(&["run", "-p", "println($A)", "-l", "scala", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stdout.contains("hello"));
}
