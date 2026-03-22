//! `axe run` — one-shot pattern search/replace.

use crate::output::OutputFormat;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use axe_core::match_tree::MatchStrictness;
use axe_core::meta_var::MetaVarEnv;
use axe_core::node::{Node, NodeMatch, Root};
use axe_language::SupportLang;
use axe_tree_sitter::doc::StrDoc;
use axe_tree_sitter::pattern::TsPattern;

/// The concrete Doc type for CLI use.
type CliDoc = StrDoc<SupportLang>;

/// Arguments for `axe run`.
#[derive(clap::Args, Debug)]
pub struct RunArgs {
    /// Pattern to search for (e.g., `console.log($A)`)
    #[arg(short, long)]
    pub pattern: String,

    /// Language (auto-detected from file extension if omitted)
    #[arg(short, long)]
    pub lang: Option<String>,

    /// Rewrite template (e.g., `logger.info($A)`)
    #[arg(short, long)]
    pub rewrite: Option<String>,

    /// Files or directories to search
    #[arg(default_value = ".")]
    pub paths: Vec<String>,

    /// Match strictness level
    #[arg(long, default_value = "smart")]
    pub strictness: String,

    /// Maximum number of results
    #[arg(long)]
    pub max_results: Option<usize>,
}

pub fn execute(args: RunArgs, format: OutputFormat) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let lang = resolve_language(&args)?;
    let strictness = parse_strictness(&args.strictness);
    let pattern = TsPattern::new(&args.pattern, &lang, lang.ts_language())?;

    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut match_count = 0u64;

    // Emit header.
    match format {
        OutputFormat::Sif => {
            writeln!(out, "#!sif v1 origin=axe/run")?;
            writeln!(out, "#schema file:str:311 line:uint:341 col:uint:341 match:str vars:str")?;
        }
        _ => {}
    }

    // Walk files.
    for base_path in &args.paths {
        let walker = ignore::WalkBuilder::new(base_path)
            .hidden(true)
            .git_ignore(true)
            .build();

        for entry in walker.flatten() {
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !lang.file_types().contains(&ext) {
                continue;
            }
            if at_limit(match_count, args.max_results) {
                break;
            }

            let src = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("{}: {e}", path.display());
                    continue;
                }
            };

            let doc = match StrDoc::new(&src, lang, lang.ts_language()) {
                Ok(d) => d,
                Err(e) => {
                    tracing::warn!("{}: parse error: {e}", path.display());
                    continue;
                }
            };
            let root = Root::new(doc);

            // Find all matches in this file using the real engine.
            let matches = root.root().find_all_by_pattern(&pattern.node, &strictness);

            for m in matches {
                if at_limit(match_count, args.max_results) {
                    break;
                }
                emit_match(path, &m, &args.rewrite, format, &mut out)?;
                match_count += 1;
            }
        }
    }

    out.flush()?;

    if match_count > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn emit_match(
    path: &Path,
    m: &NodeMatch<'_, CliDoc>,
    _rewrite: &Option<String>,
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let node = m.node();
    let line = node.start_pos().line + 1;
    let col = node.start_pos().column + 1;
    let text = node.text();
    let display_path = path.display();

    // Build vars string from captures.
    let vars = format_captures(m.env());

    match format {
        OutputFormat::Sif => {
            let escaped = sif_escape(text);
            let vars_escaped = sif_escape(&vars);
            writeln!(out, "{display_path}\t{line}\t{col}\t{escaped}\t{vars_escaped}")?;
        }
        OutputFormat::Json => {
            let json_text = json_escape(text);
            let json_vars = json_escape(&vars);
            writeln!(out,
                r#"{{"file":"{display_path}","line":{line},"column":{col},"match":"{json_text}","vars":"{json_vars}"}}"#
            )?;
        }
        _ => {
            // Plain/color output.
            let first_line = text.lines().next().unwrap_or("");
            if vars.is_empty() {
                writeln!(out, "{display_path}:{line}:{col}: {first_line}")?;
            } else {
                writeln!(out, "{display_path}:{line}:{col}: {first_line}  [{vars}]")?;
            }
        }
    }
    Ok(())
}

fn format_captures(env: &MetaVarEnv<'_, CliDoc>) -> String {
    let mut parts: Vec<String> = env
        .iter_singles()
        .map(|(name, node)| {
            let text = node.text();
            let short = if text.len() > 60 { &text[..60] } else { text };
            format!("${name}={short}")
        })
        .collect();
    for (name, nodes) in env.iter_multis() {
        let texts: Vec<&str> = nodes.iter().map(|n| n.text()).collect();
        parts.push(format!("$$${name}=[{}]", texts.join(", ")));
    }
    parts.sort(); // Deterministic output.
    parts.join(", ")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn at_limit(count: u64, max: Option<usize>) -> bool {
    max.is_some_and(|m| count >= m as u64)
}

fn resolve_language(args: &RunArgs) -> Result<SupportLang, Box<dyn std::error::Error>> {
    if let Some(ref lang_str) = args.lang {
        SupportLang::from_str(lang_str)
            .ok_or_else(|| format!("unknown language: {lang_str}").into())
    } else {
        for path_str in &args.paths {
            let path = Path::new(path_str);
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(lang) = SupportLang::from_extension(ext) {
                    return Ok(lang);
                }
            }
        }
        Err("cannot infer language — use --lang".into())
    }
}

fn parse_strictness(s: &str) -> MatchStrictness {
    match s.to_lowercase().as_str() {
        "cst" => MatchStrictness::Cst,
        "smart" => MatchStrictness::Smart,
        "ast" => MatchStrictness::Ast,
        "relaxed" => MatchStrictness::Relaxed,
        "signature" => MatchStrictness::Signature,
        "template" => MatchStrictness::Template,
        _ => MatchStrictness::Smart,
    }
}
