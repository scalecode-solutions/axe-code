//! `axe run` — one-shot pattern search/replace.

use crate::output::OutputFormat;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use axe_core::match_tree::MatchStrictness;
use axe_core::meta_var::MetaVarEnv;
use axe_core::node::{Node, NodeMatch, Root};
use axe_core::replacer;
use axe_language::SupportLang;
use axe_tree_sitter::doc::StrDoc;
use axe_tree_sitter::pattern::TsPattern;

type CliDoc = StrDoc<SupportLang>;

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

    /// Actually write changes to files (without this, rewrites are previewed)
    #[arg(long)]
    pub apply: bool,

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
    let is_rewrite = args.rewrite.is_some();

    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut match_count = 0u64;
    let mut files_changed = 0u64;

    // Emit header.
    if !is_rewrite {
        emit_search_header(format, &mut out)?;
    } else {
        emit_rewrite_header(format, &mut out)?;
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
            let matches = root.root().find_all_by_pattern(&pattern.node, &strictness);

            if matches.is_empty() {
                continue;
            }

            if let Some(ref rewrite_template) = args.rewrite {
                let new_src = apply_rewrites(&src, &matches, rewrite_template);
                emit_rewrite_result(
                    path, &src, &new_src, &matches, rewrite_template,
                    args.apply, format, &mut out,
                )?;
                if args.apply && new_src != src {
                    std::fs::write(path, &new_src)?;
                    files_changed += 1;
                }
                match_count += matches.len() as u64;
            } else {
                for m in &matches {
                    if at_limit(match_count, args.max_results) {
                        break;
                    }
                    emit_search_match(path, m, format, &mut out)?;
                    match_count += 1;
                }
            }
        }
    }

    out.flush()?;

    if is_rewrite {
        let applied = if args.apply { "applied" } else { "preview" };
        eprintln!("axe: {match_count} matches in {files_changed} files ({applied})");
    }

    if match_count > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

// ---------------------------------------------------------------------------
// Rewrite engine
// ---------------------------------------------------------------------------

/// A single replacement: byte range in the original source + replacement text.
struct Replacement {
    start: usize,
    end: usize,
    text: String,
}

/// Apply all rewrites to a source string. Matches are applied back-to-front
/// to preserve byte offsets.
fn apply_rewrites(
    src: &str,
    matches: &[NodeMatch<'_, CliDoc>],
    template: &str,
) -> String {
    let mut replacements: Vec<Replacement> = matches
        .iter()
        .map(|m| {
            let expanded = replacer::apply_template(template, '$', m.env());
            let range = m.node().range();
            Replacement {
                start: range.start,
                end: range.end,
                text: expanded,
            }
        })
        .collect();

    // Sort by start position descending so we can apply back-to-front.
    replacements.sort_by(|a, b| b.start.cmp(&a.start));

    // Remove overlapping replacements (keep the first/outermost).
    let mut result_replacements: Vec<&Replacement> = Vec::new();
    let mut min_start = usize::MAX;
    for r in &replacements {
        if r.end <= min_start {
            result_replacements.push(r);
            min_start = r.start;
        }
    }
    // Reverse so we still apply back-to-front.
    result_replacements.reverse();
    // Actually they're already in descending order, re-reverse was wrong.
    // Let's just apply in the order they are (descending start).
    result_replacements.reverse();

    let mut new_src = src.to_string();
    // Apply back-to-front (replacements sorted descending by start).
    for r in &replacements {
        if r.start < new_src.len() && r.end <= new_src.len() {
            new_src.replace_range(r.start..r.end, &r.text);
        }
    }
    new_src
}

// ---------------------------------------------------------------------------
// Output: search mode
// ---------------------------------------------------------------------------

fn emit_search_header(format: OutputFormat, out: &mut impl Write) -> std::io::Result<()> {
    if format == OutputFormat::Sif {
        writeln!(out, "#!sif v1 origin=axe/run")?;
        writeln!(out, "#schema file:str:311 line:uint:341 col:uint:341 match:str vars:str")?;
    }
    Ok(())
}

fn emit_search_match(
    path: &Path,
    m: &NodeMatch<'_, CliDoc>,
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let node = m.node();
    let line = node.start_pos().line + 1;
    let col = node.start_pos().column + 1;
    let text = node.text();
    let vars = format_captures(m.env());
    let display_path = path.display();

    match format {
        OutputFormat::Sif => {
            writeln!(out, "{display_path}\t{line}\t{col}\t{}\t{}",
                sif_escape(text), sif_escape(&vars))?;
        }
        OutputFormat::Json => {
            writeln!(out,
                r#"{{"file":"{display_path}","line":{line},"column":{col},"match":"{}","vars":"{}"}}"#,
                json_escape(text), json_escape(&vars))?;
        }
        _ => {
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

// ---------------------------------------------------------------------------
// Output: rewrite mode
// ---------------------------------------------------------------------------

fn emit_rewrite_header(format: OutputFormat, out: &mut impl Write) -> std::io::Result<()> {
    if format == OutputFormat::Sif {
        writeln!(out, "#!sif v1 origin=axe/run")?;
        writeln!(out, "#schema file:str:311 line:uint:341 col:uint:341 original:str replacement:str")?;
    }
    Ok(())
}

fn emit_rewrite_result(
    path: &Path,
    old_src: &str,
    new_src: &str,
    matches: &[NodeMatch<'_, CliDoc>],
    template: &str,
    is_apply: bool,
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_path = path.display();

    match format {
        OutputFormat::Sif => {
            for m in matches {
                let node = m.node();
                let line = node.start_pos().line + 1;
                let col = node.start_pos().column + 1;
                let original = node.text();
                let replacement = replacer::apply_template(template, '$', m.env());
                writeln!(out, "{display_path}\t{line}\t{col}\t{}\t{}",
                    sif_escape(original), sif_escape(&replacement))?;
            }
        }
        OutputFormat::Json => {
            for m in matches {
                let node = m.node();
                let line = node.start_pos().line + 1;
                let col = node.start_pos().column + 1;
                let original = node.text();
                let replacement = replacer::apply_template(template, '$', m.env());
                writeln!(out,
                    r#"{{"file":"{display_path}","line":{line},"column":{col},"original":"{}","replacement":"{}"}}"#,
                    json_escape(original), json_escape(&replacement))?;
            }
        }
        _ => {
            // Unified diff style.
            if old_src != new_src {
                let action = if is_apply { "APPLIED" } else { "PREVIEW" };
                writeln!(out, "--- {display_path} ({action})")?;
                for m in matches {
                    let node = m.node();
                    let line = node.start_pos().line + 1;
                    let original = node.text().lines().next().unwrap_or("");
                    let replacement = replacer::apply_template(template, '$', m.env());
                    let replacement_line = replacement.lines().next().unwrap_or("");
                    writeln!(out, "  {line}: - {original}")?;
                    writeln!(out, "  {line}: + {replacement_line}")?;
                }
                writeln!(out)?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
    parts.sort();
    parts.join(", ")
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
