//! `axe run` — one-shot pattern search/replace.

use crate::output::OutputFormat;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::thread;

use crossbeam_channel::bounded;

use axe_core::match_tree::MatchStrictness;
use axe_core::meta_var::MetaVarEnv;
use axe_core::node::{NodeMatch, Root};
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

/// A match result from a single file, sent through the channel.
struct FileMatchResult {
    path: PathBuf,
    matches: Vec<MatchEntry>,
    /// For rewrites: the new source code after all replacements.
    new_src: Option<String>,
    /// Original source (needed for rewrite diffs).
    old_src: Option<String>,
}

/// A single match within a file.
struct MatchEntry {
    line: usize,
    col: usize,
    text: String,
    vars: String,
    /// For rewrites: the replacement text.
    replacement: Option<String>,
}

pub fn execute(args: RunArgs, format: OutputFormat) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let lang = resolve_language(&args)?;
    let strictness = parse_strictness(&args.strictness);
    let pattern_str = args.pattern.clone();
    let is_rewrite = args.rewrite.is_some();
    let rewrite_template = args.rewrite.clone();
    let apply = args.apply;
    let max_results = args.max_results;
    let file_types: Vec<&'static str> = lang.file_types().to_vec();

    {
        let mut out = std::io::BufWriter::new(std::io::stdout().lock());
        // Emit header.
        if !is_rewrite {
            emit_search_header(format, &mut out)?;
        } else {
            emit_rewrite_header(format, &mut out)?;
        }
        out.flush()?;
    }

    let (tx, rx) = bounded::<FileMatchResult>(256);

    // Shared data for worker threads.
    let pattern_str = Arc::new(pattern_str);
    let rewrite_template = Arc::new(rewrite_template);
    let file_types = Arc::new(file_types);

    // Build parallel walker for all paths.
    let mut builder = ignore::WalkBuilder::new(&args.paths[0]);
    for p in &args.paths[1..] {
        builder.add(p);
    }
    let walker = builder
        .hidden(true)
        .git_ignore(true)
        .build_parallel();

    // Spawn receiver thread (main output).
    let output_handle = thread::spawn(move || -> Result<(u64, u64), Box<dyn std::error::Error + Send + Sync>> {
        let mut out = std::io::BufWriter::new(std::io::stdout().lock());
        let mut match_count = 0u64;
        let mut files_changed = 0u64;

        for result in rx {
            for entry in &result.matches {
                if max_results.is_some_and(|m| match_count >= m as u64) {
                    break;
                }

                if is_rewrite {
                    if let Some(ref replacement) = entry.replacement {
                        emit_rewrite_entry(
                            &result.path, entry.line, entry.col,
                            &entry.text, replacement,
                            format, &mut out,
                        )?;
                    }
                } else {
                    emit_search_entry(
                        &result.path, entry.line, entry.col,
                        &entry.text, &entry.vars,
                        format, &mut out,
                    )?;
                }
                match_count += 1;
            }

            // Apply rewrites if requested.
            if is_rewrite && apply {
                if let Some(ref new_src) = result.new_src {
                    if let Some(ref old_src) = result.old_src {
                        if new_src != old_src {
                            if let Err(e) = std::fs::write(&result.path, new_src) {
                                eprintln!("axe: error writing {}: {e}", result.path.display());
                            } else {
                                files_changed += 1;
                            }
                        }
                    }
                }
            }
        }

        out.flush()?;
        Ok((match_count, files_changed))
    });

    // Run parallel walker — each worker parses + matches files.
    walker.run(|| {
        let tx = tx.clone();
        let pattern_str = Arc::clone(&pattern_str);
        let rewrite_template = Arc::clone(&rewrite_template);
        let file_types = Arc::clone(&file_types);

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

            // Parse pattern per-thread (TsPattern is not Send).
            let pattern = match TsPattern::new(&pattern_str, &lang, lang.ts_language()) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("pattern error: {e}");
                    return ignore::WalkState::Quit;
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
            let matches = root.root().find_all_by_pattern(&pattern.node, &strictness);

            if matches.is_empty() {
                return ignore::WalkState::Continue;
            }

            let mut entries = Vec::with_capacity(matches.len());
            let mut new_src = None;

            if let Some(ref template) = *rewrite_template {
                // Compute indentation-preserving replacements.
                let rewritten = apply_rewrites(&src, &matches, template);
                for m in &matches {
                    let node = m.node();
                    let replacement = replacer::compute_replacement_utf8(template, '$', m.env(), &node);
                    entries.push(MatchEntry {
                        line: node.start_pos().line + 1,
                        col: node.start_pos().column + 1,
                        text: node.text().to_string(),
                        vars: format_captures(m.env()),
                        replacement: Some(replacement),
                    });
                }
                new_src = Some(rewritten);
            } else {
                for m in &matches {
                    let node = m.node();
                    entries.push(MatchEntry {
                        line: node.start_pos().line + 1,
                        col: node.start_pos().column + 1,
                        text: node.text().to_string(),
                        vars: format_captures(m.env()),
                        replacement: None,
                    });
                }
            }

            let _ = tx.send(FileMatchResult {
                path: path.to_path_buf(),
                matches: entries,
                new_src,
                old_src: if rewrite_template.is_some() { Some(src) } else { None },
            });

            ignore::WalkState::Continue
        })
    });
    drop(tx);

    let (match_count, files_changed) = output_handle.join().unwrap()
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;

    if is_rewrite {
        let applied = if apply { "applied" } else { "preview" };
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
/// to preserve byte offsets. Uses indentation-preserving replacement.
fn apply_rewrites(
    src: &str,
    matches: &[NodeMatch<'_, CliDoc>],
    template: &str,
) -> String {
    let mut replacements: Vec<Replacement> = matches
        .iter()
        .map(|m| {
            let expanded = replacer::compute_replacement_utf8(template, '$', m.env(), &m.node());
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

    // Remove overlapping replacements (keep the outermost).
    let mut filtered: Vec<&Replacement> = Vec::new();
    let mut min_start = usize::MAX;
    for r in &replacements {
        if r.end <= min_start {
            filtered.push(r);
            min_start = r.start;
        }
    }

    let mut new_src = src.to_string();
    // Apply in descending order (filtered is already descending by start).
    for r in &filtered {
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

fn emit_search_entry(
    path: &Path,
    line: usize,
    col: usize,
    text: &str,
    vars: &str,
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let display_path = path.display();

    match format {
        OutputFormat::Sif => {
            writeln!(out, "{display_path}\t{line}\t{col}\t{}\t{}",
                sif_escape(text), sif_escape(vars))?;
        }
        OutputFormat::Json => {
            writeln!(out,
                r#"{{"file":"{display_path}","line":{line},"column":{col},"match":"{}","vars":"{}"}}"#,
                json_escape(text), json_escape(vars))?;
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

fn emit_rewrite_entry(
    path: &Path,
    line: usize,
    col: usize,
    original: &str,
    replacement: &str,
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let display_path = path.display();

    match format {
        OutputFormat::Sif => {
            writeln!(out, "{display_path}\t{line}\t{col}\t{}\t{}",
                sif_escape(original), sif_escape(replacement))?;
        }
        OutputFormat::Json => {
            writeln!(out,
                r#"{{"file":"{display_path}","line":{line},"column":{col},"original":"{}","replacement":"{}"}}"#,
                json_escape(original), json_escape(replacement))?;
        }
        _ => {
            let orig_line = original.lines().next().unwrap_or("");
            let repl_line = replacement.lines().next().unwrap_or("");
            writeln!(out, "  {line}: - {orig_line}")?;
            writeln!(out, "  {line}: + {repl_line}")?;
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
