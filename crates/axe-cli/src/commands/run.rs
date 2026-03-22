//! `axe run` — one-shot pattern search/replace.

use crate::output::OutputFormat;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use axe_core::match_tree::MatchStrictness;
use axe_core::node::Root;
use axe_core::source::SgNode;
use axe_language::SupportLang;
use axe_tree_sitter::doc::StrDoc;
use axe_tree_sitter::pattern::TsPattern;

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

    // Compile pattern.
    let pattern = compile_pattern_for_lang(&args.pattern, lang, lang.ts_language())?;

    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    let mut match_count = 0u64;

    // Emit SIF header.
    if format == OutputFormat::Sif {
        writeln!(out, "#!sif v1 origin=axe/run")?;
        writeln!(out, "#schema file:str:311 line:uint:341 col:uint:341 text:str match:str")?;
    }

    // Walk files.
    for base_path in &args.paths {
        let walker = ignore::WalkBuilder::new(base_path)
            .hidden(true)
            .git_ignore(true)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("walk error: {e}");
                    continue;
                }
            };

            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }

            let path = entry.path();

            // Filter by language extension.
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !lang.file_types().contains(&ext) {
                continue;
            }

            // Read and parse.
            let src = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("read error {}: {e}", path.display());
                    continue;
                }
            };

            if let Some(max) = args.max_results {
                if match_count >= max as u64 {
                    break;
                }
            }

            // Search this file.
            match search_file(path, &src, lang, lang.ts_language(), &pattern, &strictness, format, &mut out, &mut match_count, args.max_results) {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!("error searching {}: {e}", path.display());
                }
            }
        }
    }

    if format == OutputFormat::Sif && match_count > 0 {
        // SIF documents end naturally — no explicit footer needed.
    }

    out.flush()?;

    if match_count > 0 {
        Ok(ExitCode::from(1)) // Non-zero = matches found (grep convention)
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn search_file(
    path: &Path,
    src: &str,
    lang: SupportLang,
    ts_lang: tree_sitter::Language,
    pattern: &TsPattern,
    strictness: &MatchStrictness,
    format: OutputFormat,
    out: &mut impl Write,
    match_count: &mut u64,
    max_results: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse with the appropriate language. We dispatch on SupportLang but
    // the StrDoc is generic over Language. Use a macro to avoid massive match.
    // For now, we parse with a minimal wrapper that just needs the ts Language.
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&ts_lang)?;
    let tree = parser.parse(src.as_bytes(), None).ok_or("parse timeout")?;
    let ts_root = axe_tree_sitter::TsNode::new(tree.root_node(), src.as_bytes());

    // DFS to find matches. We work at the TsNode level directly.
    find_matches_in_node(
        &ts_root, path, src, pattern, strictness, format, out, match_count, max_results,
    )
}

fn find_matches_in_node(
    node: &axe_tree_sitter::TsNode<'_>,
    path: &Path,
    src: &str,
    pattern: &TsPattern,
    strictness: &MatchStrictness,
    format: OutputFormat,
    out: &mut impl Write,
    match_count: &mut u64,
    max_results: Option<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    use axe_core::match_tree::match_node_impl;
    use axe_core::match_tree::MatchOneNode;

    // We need MetaVarEnv parameterized by a Doc. We use a trick: since we're
    // only doing matching (not capture extraction), we can check via the
    // raw TsNode. But for proper env, we need the Doc type. For the CLI,
    // we'll use a lightweight approach: match at the TsNode/PatternNode level.

    // Try matching this node.
    // Since match_node_impl needs Node<D> and MetaVarEnv<D>, and we want to
    // avoid the Doc type overhead for the file walker, we use a simpler
    // approach: structural comparison at the PatternNode level.
    if try_match_ts_node(node, &pattern.node, strictness) {
        if let Some(max) = max_results {
            if *match_count >= max as u64 {
                return Ok(());
            }
        }

        let (line, col) = node.start_position();
        let text = node.text();
        let display_path = path.display();

        match format {
            OutputFormat::Sif => {
                // Escape tabs in text for SIF.
                let escaped = text.replace('\t', "\\t").replace('\n', "\\n");
                let short = if escaped.len() > 80 { &escaped[..80] } else { &escaped };
                writeln!(out, "{display_path}\t{}\t{}\t{short}\t{short}", line + 1, col + 1)?;
            }
            OutputFormat::Json => {
                writeln!(out, r#"{{"file":"{}","line":{},"column":{},"text":"{}"}}"#,
                    display_path, line + 1, col + 1,
                    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
                )?;
            }
            _ => {
                // Color or other — simple for now.
                writeln!(out, "{}:{}:{}: {}", display_path, line + 1, col + 1, text.lines().next().unwrap_or(""))?;
            }
        }

        *match_count += 1;
        return Ok(());  // Don't recurse into matched node's children.
    }

    // Recurse into children.
    for child in node.children() {
        find_matches_in_node(&child, path, src, pattern, strictness, format, out, match_count, max_results)?;
        if let Some(max) = max_results {
            if *match_count >= max as u64 {
                return Ok(());
            }
        }
    }
    Ok(())
}

/// Lightweight matching: compare PatternNode against TsNode without a full Doc.
/// This avoids needing a concrete Doc type for the file walker.
fn try_match_ts_node(
    candidate: &axe_tree_sitter::TsNode<'_>,
    goal: &axe_core::match_tree::PatternNode,
    strictness: &MatchStrictness,
) -> bool {
    use axe_core::match_tree::PatternNode;
    use axe_core::meta_var::MetaVariable;
    use axe_core::source::SgNode;

    match goal {
        PatternNode::MetaVar { meta_var } => {
            match meta_var {
                MetaVariable::Capture(_, named) | MetaVariable::Anonymous(named) => {
                    !(*named && !candidate.is_named())
                }
                MetaVariable::Ellipsis | MetaVariable::MultiCapture(_) => true,
            }
        }
        PatternNode::Terminal { text, kind_id, is_named } => {
            let kind_match = *kind_id == candidate.kind_id() || *kind_id == 0xFFFF;
            if kind_match && (!*is_named || *text == candidate.text()) {
                return true;
            }
            // In non-CST modes, skip unnamed nodes.
            false
        }
        PatternNode::Internal { kind_id, children } => {
            let skip_kind = matches!(strictness, MatchStrictness::Template);
            let kind_match = skip_kind || *kind_id == candidate.kind_id() || *kind_id == 0xFFFF;
            if !kind_match {
                return false;
            }
            // Match children sequences.
            let cand_children = candidate.children();
            match_children_ts(&children, &cand_children, strictness)
        }
    }
}

fn match_children_ts(
    goals: &[axe_core::match_tree::PatternNode],
    candidates: &[axe_tree_sitter::TsNode<'_>],
    strictness: &MatchStrictness,
) -> bool {
    use axe_core::match_tree::PatternNode;
    use axe_core::source::SgNode;

    let mut gi = 0;
    let mut ci = 0;

    while gi < goals.len() {
        let goal = &goals[gi];

        // Handle ellipsis.
        if goal.is_ellipsis() {
            gi += 1;
            // Skip trivial goals after ellipsis.
            while gi < goals.len() && goals[gi].is_trivial() {
                gi += 1;
            }
            // If no more goals, ellipsis consumes rest.
            if gi >= goals.len() {
                return true;
            }
            // Find next anchor match.
            while ci < candidates.len() {
                if try_match_ts_node(&candidates[ci], &goals[gi], strictness) {
                    break;
                }
                ci += 1;
            }
            if ci >= candidates.len() {
                return false;
            }
            continue;
        }

        // Non-ellipsis: match current goal with current candidate.
        loop {
            if ci >= candidates.len() {
                // Candidates exhausted — remaining goals must be trivial.
                return goals[gi..].iter().all(|g| g.is_trivial());
            }
            let cand = &candidates[ci];

            if try_match_ts_node(cand, goal, strictness) {
                gi += 1;
                ci += 1;
                break;
            }

            // Skip logic based on strictness.
            // In Smart mode: unnamed terminals in the pattern are compared;
            // only unnamed candidates not in the pattern can be skipped.
            // In AST/Relaxed: both unnamed goals and unnamed candidates skip.
            let goal_unnamed = goal.is_trivial();
            let cand_unnamed = !cand.is_named();

            let skip_unnamed_goals = matches!(
                strictness,
                MatchStrictness::Ast | MatchStrictness::Relaxed | MatchStrictness::Signature
            );

            if goal_unnamed && cand_unnamed && skip_unnamed_goals {
                // Both unnamed and in a mode that skips them.
                gi += 1;
                ci += 1;
                break;
            } else if goal_unnamed && skip_unnamed_goals {
                gi += 1;
                break;
            } else if cand_unnamed && !matches!(strictness, MatchStrictness::Cst) && !goal_unnamed {
                // Candidate is unnamed filler (like whitespace token) — skip it.
                ci += 1;
            } else {
                return false;
            }
        }
    }

    // All goals matched. Remaining candidates must be trailing/skippable.
    candidates[ci..].iter().all(|c| {
        !c.is_named() || (matches!(strictness, MatchStrictness::Relaxed | MatchStrictness::Signature) && c.kind().contains("comment"))
    })
}

fn resolve_language(args: &RunArgs) -> Result<SupportLang, Box<dyn std::error::Error>> {
    if let Some(ref lang_str) = args.lang {
        SupportLang::from_str(lang_str)
            .ok_or_else(|| format!("unknown language: {lang_str}").into())
    } else {
        // Try to infer from the first path's extension.
        for path_str in &args.paths {
            let path = Path::new(path_str);
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(lang) = SupportLang::from_extension(ext) {
                    return Ok(lang);
                }
            }
            // If it's a directory, can't infer — require --lang.
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

fn compile_pattern_for_lang(
    pattern: &str,
    lang: SupportLang,
    ts_lang: tree_sitter::Language,
) -> Result<TsPattern, Box<dyn std::error::Error>> {
    // We need a concrete Language impl. Dispatch based on SupportLang.
    // All built-in languages use the same macro-generated impls with
    // identical pre_process_pattern logic (replace $ with µ), so we
    // can use JavaScript as a stand-in for the pre-processing.
    // The actual parsing uses the correct ts_lang.
    let pat = TsPattern::new(pattern, &axe_language::JavaScript, ts_lang)?;
    Ok(pat)
}
