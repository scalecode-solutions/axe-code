//! CHEVRON 3: THE KAWOOSH
//!
//! Broken, malformed, incomplete, and nonsensical input.
//! Does axe report an error, or does it die?

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
// Empty input
// ======================================================================

#[test]
fn empty_pattern() {
    let f = write_tmp("k_empty.js", "const x = 42;");
    let (_, _, code) = axe(&["run", "-p", "", "-l", "js", f.to_str().unwrap()]);
    // Should error gracefully, not crash.
    assert!(code == 0 || code == 1 || code == 2, "should not crash on empty pattern, got code {code}");
}

#[test]
fn empty_file() {
    let f = write_tmp("k_emptyfile.js", "");
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert_eq!(code, 0, "empty file should produce no matches");
}

#[test]
fn whitespace_only_file() {
    let f = write_tmp("k_ws.js", "   \n\n  \t\n  ");
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert_eq!(code, 0, "whitespace-only file should produce no matches");
}

// ======================================================================
// Malformed source code
// ======================================================================

#[test]
fn unclosed_string() {
    let f = write_tmp("k_unclosed.js", "const x = \"hello");
    let (_, _, code) = axe(&["run", "-p", "const $A = $B", "-l", "js", f.to_str().unwrap()]);
    // tree-sitter recovers from parse errors — should not crash.
    assert!(code == 0 || code == 1, "should not crash on unclosed string");
}

#[test]
fn unclosed_bracket() {
    let f = write_tmp("k_bracket.js", "function foo() { if (true) { console.log(");
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert!(code == 0 || code == 1, "should not crash on unclosed bracket");
}

#[test]
fn completely_invalid_syntax() {
    let f = write_tmp("k_garbage.js", "}{}{}{!!!@@@###$$$%%%^^^&&&***((()))");
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert!(code == 0 || code == 1, "should not crash on garbage");
}

#[test]
fn binary_content() {
    let dir = std::env::temp_dir().join("axe_furling");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("k_binary.js");
    std::fs::write(&f, &[0u8, 1, 2, 3, 0xFF, 0xFE, 0xFD, 128, 129, 200]).unwrap();
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert!(code == 0 || code == 1, "should not crash on binary content");
}

// ======================================================================
// Unicode edge cases
// ======================================================================

#[test]
fn emoji_in_source() {
    let f = write_tmp("k_emoji.js", "console.log('🎉🔥💀');");
    let (stdout, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert_eq!(code, 1, "should match even with emoji");
    assert!(stdout.contains("$A="), "should capture emoji string");
}

#[test]
fn cjk_in_source() {
    let f = write_tmp("k_cjk.js", "console.log('你好世界');");
    let (stdout, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stdout.contains("$A="));
}

#[test]
fn null_bytes_in_source() {
    let dir = std::env::temp_dir().join("axe_furling");
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("k_null.js");
    std::fs::write(&f, b"console.log(\x00'hello');").unwrap();
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    // Should not crash.
    assert!(code == 0 || code == 1, "should not crash on null bytes");
}

#[test]
fn rtl_override_in_source() {
    let f = write_tmp("k_rtl.js", "console.log('\u{202E}hello');");
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert!(code == 0 || code == 1, "should not crash on RTL override");
}

#[test]
fn zero_width_chars() {
    let f = write_tmp("k_zw.js", "console\u{200B}.log('hello');"); // zero-width space
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    // May or may not match (zero-width char breaks the identifier), but should not crash.
    assert!(code == 0 || code == 1, "should not crash on zero-width chars");
}

// ======================================================================
// Invalid arguments
// ======================================================================

#[test]
fn unknown_language() {
    let f = write_tmp("k_unknownlang.txt", "hello");
    let (_, _, code) = axe(&["run", "-p", "$A", "-l", "klingon", f.to_str().unwrap()]);
    assert_ne!(code, 0, "unknown language should error");
}

#[test]
fn missing_pattern() {
    let f = write_tmp("k_nopat.js", "hello");
    let (_, _, code) = axe(&["run", "-l", "js", f.to_str().unwrap()]);
    assert_ne!(code, 0, "missing -p should error");
}

#[test]
fn nonexistent_file() {
    let (_, _, code) = axe(&["run", "-p", "$A", "-l", "js", "/tmp/axe_does_not_exist_12345.js"]);
    assert_eq!(code, 0, "missing file should produce 0 matches, not crash");
}

#[test]
fn nonexistent_directory() {
    let (_, _, code) = axe(&["run", "-p", "$A", "-l", "js", "/tmp/axe_no_dir_12345/"]);
    // Should not crash.
    assert!(code == 0 || code == 1);
}

// ======================================================================
// Huge pattern
// ======================================================================

#[test]
fn very_long_pattern() {
    let pattern = "console.log(".to_string() + &"$A, ".repeat(100) + "$Z)";
    let f = write_tmp("k_longpat.js", "console.log(1);");
    let (_, _, code) = axe(&["run", "-p", &pattern, "-l", "js", f.to_str().unwrap()]);
    // May fail to compile pattern, but should not crash.
    assert!(code == 0 || code == 1 || code == 2, "long pattern should not crash, got {code}");
}

// ======================================================================
// Enormous file
// ======================================================================

#[test]
fn large_file() {
    let content = "console.log('line');\n".repeat(10_000);
    let f = write_tmp("k_large.js", &content);
    let (_, _, code) = axe(&["run", "-p", "console.log($A)", "-l", "js", "--max-results", "5", f.to_str().unwrap()]);
    assert_eq!(code, 1, "should find matches in large file");
}

// ======================================================================
// Pattern that is valid syntax but semantically weird
// ======================================================================

#[test]
fn pattern_is_just_a_metavar() {
    let f = write_tmp("k_metavar.js", "const x = 42;");
    let (_, _, code) = axe(&["run", "-p", "$A", "-l", "js", f.to_str().unwrap()]);
    // $A matches anything — may produce many results or error. Just don't crash.
    assert!(code == 0 || code == 1, "bare metavar pattern should not crash");
}

#[test]
fn pattern_is_just_ellipsis() {
    let f = write_tmp("k_ellipsis.js", "const x = 42;");
    let (_, _, code) = axe(&["run", "-p", "$$$", "-l", "js", f.to_str().unwrap()]);
    assert!(code == 0 || code == 1, "bare ellipsis should not crash");
}

// ======================================================================
// Malformed rule JSON
// ======================================================================

#[test]
fn scan_with_invalid_json() {
    let f = write_tmp("k_badjson.js", "const x = 42;");
    let (_, _, code) = axe(&["scan", "--inline-rules", "not json at all", f.to_str().unwrap()]);
    assert_ne!(code, 0, "invalid JSON should error");
}

#[test]
fn scan_with_empty_rule() {
    let f = write_tmp("k_emptyrule.js", "const x = 42;");
    let (_, _, code) = axe(&["scan", "--inline-rules", "{}", f.to_str().unwrap()]);
    // Missing required fields — should error, not crash.
    assert_ne!(code, 0, "empty rule should error");
}

#[test]
fn test_rule_with_no_tests() {
    let rule = write_tmp("k_notests.json", r#"{
        "id": "no-tests",
        "language": "javascript",
        "rule": { "pattern": "console.log($A)" },
        "severity": "Warning",
        "message": "test"
    }"#);
    let (_, _, code) = axe(&["test", "-r", rule.to_str().unwrap()]);
    // No tests to run — should exit 0, not crash.
    assert_eq!(code, 0);
}
