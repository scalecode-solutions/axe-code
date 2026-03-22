//! `axe scan` — config-driven multi-rule scanning.

use crate::output::OutputFormat;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;

use crossbeam_channel::bounded;

use axe_config::{CombinedScan, CompileContext, RuleConfig, Severity, compile_rule};
use axe_core::node::Root;
use axe_language::SupportLang;
use axe_tree_sitter::doc::StrDoc;
use axe_tree_sitter::pattern::TsPattern;

#[derive(clap::Args, Debug)]
pub struct ScanArgs {
    /// Rule file(s) or directory containing rules (JSON format)
    #[arg(short, long)]
    pub rule: Vec<String>,

    /// Inline rule as JSON string
    #[arg(long)]
    pub inline_rules: Option<String>,

    /// Files or directories to scan
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    /// Filter by minimum severity (hint, info, warning, error)
    #[arg(long)]
    pub severity: Option<String>,

    /// Maximum number of results
    #[arg(long)]
    pub max_results: Option<usize>,
}

/// A scan hit sent through the channel from worker threads.
struct ScanResult {
    path: PathBuf,
    hits: Vec<ScanHitEntry>,
}

/// A single scan hit entry extracted from the tree (thread-safe, owns its data).
struct ScanHitEntry {
    rule_idx: usize,
    line: usize,
    col: usize,
    text: String,
}

pub fn execute(args: ScanArgs, format: OutputFormat) -> Result<ExitCode, Box<dyn std::error::Error>> {
    // Load rule configs.
    let configs = load_rule_configs(&args)?;
    if configs.is_empty() {
        eprintln!("axe scan: no rules loaded. Use --rule <file> or --inline-rules.");
        return Ok(ExitCode::SUCCESS);
    }

    let min_severity = parse_severity_filter(&args.severity);

    // Group rules by language.
    let mut lang_rules: std::collections::HashMap<String, Vec<(axe_config::Rule, &RuleConfig)>> =
        std::collections::HashMap::new();

    for config in &configs {
        let lang = SupportLang::from_str(&config.language)
            .ok_or_else(|| format!("unknown language in rule {}: {}", config.id, config.language))?;
        let ctx = CompileContext {
            compile_pattern: |pattern: &str| -> Result<(axe_core::match_tree::PatternNode, Option<bit_set::BitSet>), String> {
                let pat = TsPattern::new(pattern, &lang, lang.ts_language())
                    .map_err(|e| e.to_string())?;
                let kinds = pat.potential_kinds();
                Ok((pat.node, kinds))
            },
            resolve_kind: |kind: &str| -> Option<u16> {
                use axe_core::language::Language;
                lang.kind_to_id(kind)
            },
        };

        let compiled = compile_rule(&config.rule, &ctx)
            .map_err(|e| format!("rule {}: {e}", config.id))?;

        lang_rules
            .entry(config.language.clone())
            .or_default()
            .push((compiled, config));
    }

    let max_results = args.max_results;

    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut total_hits = 0u64;

    // Emit header.
    match format {
        OutputFormat::Sif => {
            writeln!(out, "#!sif v1 origin=axe/scan")?;
            writeln!(out, "#schema file:str:311 line:uint:341 col:uint:341 rule:str severity:str message:str match:str")?;
        }
        _ => {}
    }

    // For each language, build a CombinedScan and walk files in parallel.
    for (lang_str, rules) in &lang_rules {
        let lang = SupportLang::from_str(lang_str).unwrap();
        let scanner = Arc::new(CombinedScan::new(rules.clone()));

        // Collect rule metadata for output (so we can use it from the receiver thread).
        let rule_meta: Vec<(String, Severity, Option<String>)> = (0..rules.len())
            .map(|i| {
                (
                    scanner.rule_id(i).to_string(),
                    scanner.severity(i),
                    scanner.message(i).map(|s| s.to_string()),
                )
            })
            .collect();
        let rule_meta = Arc::new(rule_meta);

        let file_types: Vec<&'static str> = lang.file_types().to_vec();
        let file_types = Arc::new(file_types);

        let (tx, rx) = bounded::<ScanResult>(256);

        // Build parallel walker for all paths.
        let mut builder = ignore::WalkBuilder::new(&args.paths[0]);
        for p in &args.paths[1..] {
            builder.add(p);
        }
        let walker = builder
            .hidden(true)
            .git_ignore(true)
            .build_parallel();

        // Spawn receiver thread for output.
        let rule_meta_recv = Arc::clone(&rule_meta);
        let output_handle = thread::spawn(move || -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
            let mut out = std::io::BufWriter::new(std::io::stdout().lock());
            let mut hit_count = 0u64;

            for result in rx {
                for entry in &result.hits {
                    let (ref rule_id, severity, ref message) = rule_meta_recv[entry.rule_idx];
                    if !meets_severity(severity, min_severity) {
                        continue;
                    }
                    if max_results.is_some_and(|m| hit_count >= m as u64) {
                        break;
                    }
                    let msg = message.as_deref().unwrap_or(rule_id);
                    emit_scan_entry(
                        &result.path, entry.line, entry.col,
                        rule_id, severity, msg, &entry.text,
                        format, &mut out,
                    )?;
                    hit_count += 1;
                }
            }

            out.flush()?;
            Ok(hit_count)
        });

        // Run parallel walker.
        walker.run(|| {
            let tx = tx.clone();
            let file_types = Arc::clone(&file_types);
            let scanner = Arc::clone(&scanner);

            Box::new(move |entry| {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => return ignore::WalkState::Continue,
                };

                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    return ignore::WalkState::Continue;
                }

                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !file_types.contains(&ext) {
                    return ignore::WalkState::Continue;
                }

                let src = match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("{}: {e}", path.display());
                        return ignore::WalkState::Continue;
                    }
                };

                let doc = match StrDoc::new(&src, lang, lang.ts_language()) {
                    Ok(d) => d,
                    Err(e) => {
                        tracing::warn!("{}: parse error: {e}", path.display());
                        return ignore::WalkState::Continue;
                    }
                };
                let root = Root::new(doc);
                let hits = scanner.scan(&root.root());

                if hits.is_empty() {
                    return ignore::WalkState::Continue;
                }

                let entries: Vec<ScanHitEntry> = hits
                    .iter()
                    .map(|hit| {
                        let node = hit.node_match.node();
                        ScanHitEntry {
                            rule_idx: hit.rule_idx,
                            line: node.start_pos().line + 1,
                            col: node.start_pos().column + 1,
                            text: node.text().to_string(),
                        }
                    })
                    .collect();

                let _ = tx.send(ScanResult {
                    path: path.to_path_buf(),
                    hits: entries,
                });

                ignore::WalkState::Continue
            })
        });
        drop(tx);

        let hits = output_handle.join().unwrap()
            .map_err(|e| -> Box<dyn std::error::Error> { e })?;
        total_hits += hits;
    }

    out.flush()?;
    eprintln!("axe scan: {total_hits} issues found ({} rules)", configs.len());

    if total_hits > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

// ---------------------------------------------------------------------------
// Rule loading
// ---------------------------------------------------------------------------

fn load_rule_configs(args: &ScanArgs) -> Result<Vec<RuleConfig>, Box<dyn std::error::Error>> {
    let mut configs = Vec::new();

    // Load from --rule files/dirs.
    for rule_path_str in &args.rule {
        load_rules_from_path(Path::new(rule_path_str), &mut configs)?;
    }

    // Load from --inline-rules.
    if let Some(ref inline) = args.inline_rules {
        configs.push(load_rule_from_json(inline)?);
    }

    // If no explicit rules, try auto-discovering from axeconfig.json.
    if configs.is_empty() {
        let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if let Some((project_root, project_config)) = axe_config::ProjectConfig::discover(&start) {
            eprintln!("axe: using config from {}", project_root.join("axeconfig.json").display());
            for dir in &project_config.rule_dirs {
                let full = project_root.join(dir);
                if full.is_dir() {
                    load_rules_from_path(&full, &mut configs)?;
                } else {
                    tracing::warn!("rule_dirs entry not found: {}", full.display());
                }
            }
            for file in &project_config.rules {
                let full = project_root.join(file);
                load_rules_from_path(&full, &mut configs)?;
            }
        }
    }

    Ok(configs)
}

fn load_rules_from_path(path: &Path, configs: &mut Vec<RuleConfig>) -> Result<(), Box<dyn std::error::Error>> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "json") {
                let content = std::fs::read_to_string(&p)?;
                match load_rule_from_json(&content) {
                    Ok(config) => configs.push(config),
                    Err(e) => tracing::warn!("{}: {e}", p.display()),
                }
            }
        }
    } else if path.exists() {
        let content = std::fs::read_to_string(path)?;
        configs.push(load_rule_from_json(&content)
            .map_err(|e| format!("{}: {e}", path.display()))?);
    } else {
        return Err(format!("rule path not found: {}", path.display()).into());
    }
    Ok(())
}

fn load_rule_from_json(json: &str) -> Result<RuleConfig, Box<dyn std::error::Error>> {
    let config: RuleConfig = forma_json::from_str(json)?;
    Ok(config)
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn emit_scan_entry(
    path: &Path,
    line: usize,
    col: usize,
    rule_id: &str,
    severity: Severity,
    message: &str,
    text: &str,
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let display_path = path.display();
    let sev_str = severity_str(severity);

    match format {
        OutputFormat::Sif => {
            let escaped_text = sif_escape(text);
            let escaped_msg = sif_escape(message);
            writeln!(out, "{display_path}\t{line}\t{col}\t{rule_id}\t{sev_str}\t{escaped_msg}\t{escaped_text}")?;
        }
        OutputFormat::Json => {
            writeln!(out,
                r#"{{"file":"{}","line":{},"column":{},"rule":"{}","severity":"{}","message":"{}","match":"{}"}}"#,
                display_path, line, col, rule_id, sev_str,
                json_escape(message), json_escape(text))?;
        }
        OutputFormat::Github => {
            let level = match severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                _ => "notice",
            };
            writeln!(out, "::{level} file={display_path},line={line},col={col}::{message} ({rule_id})")?;
        }
        _ => {
            let sev_marker = match severity {
                Severity::Error => "E",
                Severity::Warning => "W",
                Severity::Info => "I",
                Severity::Hint => "H",
            };
            let first_line = text.lines().next().unwrap_or("");
            writeln!(out, "{display_path}:{line}:{col}: {sev_marker}[{rule_id}] {message}")?;
            writeln!(out, "  {first_line}")?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Hint => "hint",
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

fn parse_severity_filter(s: &Option<String>) -> Severity {
    match s.as_deref() {
        Some("error") => Severity::Error,
        Some("warning") | Some("warn") => Severity::Warning,
        Some("info") => Severity::Info,
        Some("hint") => Severity::Hint,
        _ => Severity::Hint, // Show everything by default.
    }
}

fn meets_severity(actual: Severity, minimum: Severity) -> bool {
    let level = |s: Severity| -> u8 {
        match s {
            Severity::Hint => 0,
            Severity::Info => 1,
            Severity::Warning => 2,
            Severity::Error => 3,
        }
    };
    level(actual) >= level(minimum)
}

fn sif_escape(s: &str) -> String {
    let r = s.replace('\t', "\\t").replace('\n', "\\n");
    if r.len() > 120 { r[..120].to_string() + "..." } else { r }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

