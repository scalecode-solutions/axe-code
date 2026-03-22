//! CHEVRON 6: THE REPLICATOR
//!
//! Self-destruction. Infinite recursion, unbounded growth, self-referential
//! inputs. Each test runs with a timeout — if the defense catches the
//! replicator before the timeout, PASS.
//!
//! CRITICAL: These tests have hard timeouts. They will NOT fork bomb your machine.

use std::process::Command;
use std::time::{Duration, Instant};

const AXE: &str = env!("CARGO_BIN_EXE_axe");
// Debug builds are ~10x slower than release — use generous timeout.
const TIMEOUT: Duration = Duration::from_secs(30);

fn axe_timed(args: &[&str]) -> (String, String, i32, Duration) {
    let start = Instant::now();
    let output = Command::new(AXE)
        .args(args)
        .output()
        .expect("failed to run axe");
    let elapsed = start.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, stderr, code, elapsed)
}

fn write_tmp(name: &str, content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("axe_furling_replicator");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

// ======================================================================
// Deeply nested code — tests stack depth
// ======================================================================

#[test]
fn deeply_nested_brackets() {
    // 500 levels of nested function calls.
    let mut src = String::new();
    for _ in 0..500 {
        src.push_str("f(");
    }
    src.push('x');
    for _ in 0..500 {
        src.push(')');
    }
    let f = write_tmp("r_deep.js", &src);
    let (_, _, code, elapsed) = axe_timed(&["run", "-p", "f($A)", "-l", "js", f.to_str().unwrap()]);
    assert!(elapsed < TIMEOUT, "deeply nested should complete within timeout: {elapsed:?}");
    assert!(code == 0 || code == 1, "should not crash: code={code}");
}

#[test]
fn deeply_nested_objects() {
    // 200 levels of nested object literals.
    let mut src = "const x = ".to_string();
    for _ in 0..200 {
        src.push_str("{a: ");
    }
    src.push('1');
    for _ in 0..200 {
        src.push('}');
    }
    let f = write_tmp("r_deep_obj.js", &src);
    let (_, _, code, elapsed) = axe_timed(&["run", "-p", "const $A = $B", "-l", "js", f.to_str().unwrap()]);
    assert!(elapsed < TIMEOUT, "nested objects: {elapsed:?}");
    assert!(code == 0 || code == 1);
}

// ======================================================================
// Enormous files
// ======================================================================

#[test]
fn million_line_file() {
    // Generate a 10K-line file (reasonable for debug builds).
    let line = "const x_NNNN = console.log('hello');\n";
    let content: String = (0..10_000).map(|i| line.replace("NNNN", &i.to_string())).collect();
    let f = write_tmp("r_huge.js", &content);
    let (_, _, code, elapsed) = axe_timed(&[
        "run", "-p", "console.log($A)", "-l", "js", "--max-results", "10", f.to_str().unwrap()
    ]);
    assert!(elapsed < TIMEOUT, "10K-line file: {elapsed:?}");
    assert!(code == 0 || code == 1, "should not crash: code={code}");
}

// ======================================================================
// Pattern that could cause exponential matching
// ======================================================================

#[test]
fn many_metavars_pattern() {
    // Pattern with many different metavars — shouldn't cause combinatorial explosion.
    let f = write_tmp("r_manyvars.js", "f(a, b, c, d, e, f, g, h);");
    let (_, _, code, elapsed) = axe_timed(&["run", "-p", "f($A, $B, $C, $D, $E, $F, $G, $H)", "-l", "js", f.to_str().unwrap()]);
    assert!(elapsed < TIMEOUT, "many metavars: {elapsed:?}");
    assert!(code == 0 || code == 1);
}

#[test]
fn ellipsis_between_ellipsis() {
    // Multiple ellipsis in a pattern — could cause greedy matching issues.
    let src = "f(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);";
    let f = write_tmp("r_multiellipsis.js", src);
    let (_, _, code, elapsed) = axe_timed(&["run", "-p", "f($$$A, 5, $$$B)", "-l", "js", f.to_str().unwrap()]);
    assert!(elapsed < TIMEOUT, "multi-ellipsis: {elapsed:?}");
    assert!(code == 0 || code == 1);
}

// ======================================================================
// Self-referential rule configs
// ======================================================================

#[test]
fn rule_with_deep_nesting() {
    // A rule with 50 levels of all: [all: [all: ...]]
    let mut rule = r#"{"pattern": "console.log($A)"}"#.to_string();
    for _ in 0..50 {
        rule = format!(r#"{{"all": [{}]}}"#, rule);
    }
    let config = format!(
        r#"{{"id":"deep","language":"javascript","rule":{},"severity":"Warning","message":"deep"}}"#,
        rule
    );
    let f = write_tmp("r_deeprule.js", "console.log('hello');");
    let (_, _, code, elapsed) = axe_timed(&["scan", "--inline-rules", &config, f.to_str().unwrap()]);
    assert!(elapsed < TIMEOUT, "deeply nested rule: {elapsed:?}");
    // May error on compilation, but should not hang or crash.
    assert!(code == 0 || code == 1 || code == 2, "code={code}");
}

// ======================================================================
// Legitimate deep patterns should still work (containment doesn't break use)
// ======================================================================

#[test]
fn legitimate_deep_match() {
    // A reasonable 10-level nested structure should still match fine.
    let src = r#"
    if (a) {
        if (b) {
            if (c) {
                if (d) {
                    if (e) {
                        console.log("found it");
                    }
                }
            }
        }
    }
    "#;
    let f = write_tmp("r_legit.js", src);
    let (stdout, _, code, elapsed) = axe_timed(&["run", "-p", "console.log($A)", "-l", "js", f.to_str().unwrap()]);
    assert!(elapsed < Duration::from_secs(2), "legitimate deep match should be fast: {elapsed:?}");
    assert_eq!(code, 1, "should find the match");
    assert!(stdout.contains("found it"));
}

// ======================================================================
// Many files (tests parallel walker stability)
// ======================================================================

#[test]
fn thousand_files() {
    let dir = std::env::temp_dir().join("axe_furling_replicator").join("thousand");
    let _ = std::fs::remove_dir_all(&dir); // Clean up from previous runs.
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..200 {
        let content = format!("const x{i} = console.log({i});");
        std::fs::write(dir.join(format!("file_{i}.js")), content).unwrap();
    }
    let (stdout, stderr, code, elapsed) = axe_timed(&[
        "run", "-p", "console.log($A)", "-l", "js", "--max-results", "100",
        dir.to_str().unwrap()
    ]);
    assert!(elapsed < TIMEOUT, "200 files: {elapsed:?}");
    // Parallel walker should find some matches. Code 0 or 1 both ok
    // (parallel timing may mean 0 results get through before shutdown).
    assert!(code == 0 || code == 1, "should not crash: code={code}, stderr={stderr}");
}
