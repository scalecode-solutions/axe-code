//! `axe test` — rule testing and snapshot validation.
//!
//! Each rule file can include a `tests` section with `valid` and `invalid`
//! code snippets. `axe test` compiles each rule, runs its test cases, and
//! reports pass/fail:
//!
//! - `valid` snippets must NOT trigger the rule (they're "good" code).
//! - `invalid` snippets MUST trigger the rule (they're "bad" code).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use axe_config::{CompileContext, RuleConfig, compile_rule};
use axe_core::language::Language;
use axe_core::node::Root;
use axe_language::SupportLang;
use axe_tree_sitter::doc::StrDoc;
use axe_tree_sitter::pattern::TsPattern;

type CliDoc = StrDoc<SupportLang>;

#[derive(clap::Args, Debug)]
pub struct TestArgs {
    /// Update all snapshots (not yet implemented)
    #[arg(long)]
    pub update_all: bool,

    /// Rule file(s) or directory to test
    #[arg(short, long)]
    pub rule: Vec<String>,

    /// Test only rules matching this ID pattern
    #[arg(long)]
    pub filter: Option<String>,
}

struct TestResult {
    rule_id: String,
    passed: usize,
    failed: usize,
    errors: Vec<String>,
}

pub fn execute(args: TestArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let configs = load_test_configs(&args)?;
    if configs.is_empty() {
        eprintln!("axe test: no rules with test cases found.");
        return Ok(ExitCode::SUCCESS);
    }

    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut total_rules = 0usize;

    for config in &configs {
        if let Some(ref filter) = args.filter {
            if !config.id.contains(filter.as_str()) {
                continue;
            }
        }

        let tests = match &config.tests {
            Some(t) if !t.valid.is_empty() || !t.invalid.is_empty() => t,
            _ => continue,
        };

        total_rules += 1;
        let result = run_rule_tests(config);

        // Display results.
        let status = if result.failed == 0 { "PASS" } else { "FAIL" };
        writeln!(out, "{status}  {}: {} passed, {} failed",
            result.rule_id, result.passed, result.failed)?;

        for err in &result.errors {
            writeln!(out, "      {err}")?;
        }

        total_passed += result.passed;
        total_failed += result.failed;
    }

    writeln!(out)?;
    writeln!(out, "Rules: {total_rules}  Passed: {total_passed}  Failed: {total_failed}")?;
    out.flush()?;

    if total_failed > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn run_rule_tests(config: &RuleConfig) -> TestResult {
    let mut result = TestResult {
        rule_id: config.id.clone(),
        passed: 0,
        failed: 0,
        errors: Vec::new(),
    };

    let lang = match SupportLang::from_str(&config.language) {
        Some(l) => l,
        None => {
            result.errors.push(format!("unknown language: {}", config.language));
            result.failed += 1;
            return result;
        }
    };

    let ctx = CompileContext {
        compile_pattern: |pattern: &str| {
            let pat = TsPattern::new(pattern, &lang, lang.ts_language())
                .map_err(|e| e.to_string())?;
            let kinds = pat.potential_kinds();
            Ok((pat.node, kinds))
        },
        resolve_kind: |kind: &str| lang.kind_to_id(kind),
    };

    let compiled = match compile_rule(&config.rule, &ctx) {
        Ok(r) => r,
        Err(e) => {
            result.errors.push(format!("compile error: {e}"));
            result.failed += 1;
            return result;
        }
    };

    let tests = config.tests.as_ref().unwrap();

    // Valid cases: rule should NOT match (code is correct).
    for (i, snippet) in tests.valid.iter().enumerate() {
        match check_snippet(&snippet, lang, &compiled, false) {
            Ok(()) => result.passed += 1,
            Err(msg) => {
                result.failed += 1;
                result.errors.push(format!("valid[{i}]: {msg}"));
            }
        }
    }

    // Invalid cases: rule SHOULD match (code is bad).
    for (i, snippet) in tests.invalid.iter().enumerate() {
        match check_snippet(&snippet, lang, &compiled, true) {
            Ok(()) => result.passed += 1,
            Err(msg) => {
                result.failed += 1;
                result.errors.push(format!("invalid[{i}]: {msg}"));
            }
        }
    }

    result
}

/// Check a single test snippet.
/// `expect_match`: true if the rule should fire, false if it should not.
fn check_snippet(
    snippet: &str,
    lang: SupportLang,
    rule: &axe_config::Rule,
    expect_match: bool,
) -> Result<(), String> {
    let doc = StrDoc::new(snippet, lang, lang.ts_language())
        .map_err(|e| format!("parse error: {e}"))?;
    let root = Root::new(doc);

    // DFS looking for any match.
    let mut found = false;
    for node in root.root().dfs() {
        if rule.match_node(node).is_some() {
            found = true;
            break;
        }
    }

    if expect_match && !found {
        let short = if snippet.len() > 50 { &snippet[..50] } else { snippet };
        Err(format!("expected match but none found: `{short}`"))
    } else if !expect_match && found {
        let short = if snippet.len() > 50 { &snippet[..50] } else { snippet };
        Err(format!("expected no match but rule fired: `{short}`"))
    } else {
        Ok(())
    }
}

fn load_test_configs(args: &TestArgs) -> Result<Vec<RuleConfig>, Box<dyn std::error::Error>> {
    let mut configs = Vec::new();

    if args.rule.is_empty() {
        // Auto-discover from project config.
        let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if let Some((root, project)) = axe_config::ProjectConfig::discover(&start) {
            for dir in &project.rule_dirs {
                load_test_rules_from_dir(&root.join(dir), &mut configs)?;
            }
            for file in &project.rules {
                load_test_rule_file(&root.join(file), &mut configs)?;
            }
        } else {
            eprintln!("axe test: no --rule specified and no axeconfig.json found.");
        }
    } else {
        for path_str in &args.rule {
            let path = Path::new(path_str);
            if path.is_dir() {
                load_test_rules_from_dir(path, &mut configs)?;
            } else {
                load_test_rule_file(path, &mut configs)?;
            }
        }
    }

    Ok(configs)
}

fn load_test_rules_from_dir(dir: &Path, configs: &mut Vec<RuleConfig>) -> Result<(), Box<dyn std::error::Error>> {
    if !dir.is_dir() { return Ok(()); }
    for entry in std::fs::read_dir(dir)? {
        let p = entry?.path();
        if p.extension().is_some_and(|e| e == "json") {
            load_test_rule_file(&p, configs)?;
        }
    }
    Ok(())
}

fn load_test_rule_file(path: &Path, configs: &mut Vec<RuleConfig>) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() { return Ok(()); }
    let content = std::fs::read_to_string(path)?;
    let config: RuleConfig = forma_json::from_str(&content)?;
    if config.tests.is_some() {
        configs.push(config);
    }
    Ok(())
}
