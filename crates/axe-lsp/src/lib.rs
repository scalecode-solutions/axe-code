//! Language Server Protocol implementation for axe.
//!
//! Improvements over ast-grep's LSP:
//! - Incremental parsing (tree-sitter tree.edit + re-parse) — planned
//! - Structured logging via tracing
//! - Proper error propagation to client

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axe_config::{
    CombinedScan, CompileContext, ProjectConfig, Rule, RuleConfig, Severity, compile_rule,
};
use axe_core::language::Language;
use axe_core::node::Root;
use axe_language::SupportLang;
use axe_tree_sitter::doc::StrDoc;
use axe_tree_sitter::pattern::TsPattern;

use dashmap::DashMap;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

type CliDoc = StrDoc<SupportLang>;

/// Per-document state.
struct DocState {
    version: i32,
    content: String,
    lang: Option<SupportLang>,
}

/// The axe language server backend.
pub struct Backend {
    client: Client,
    documents: DashMap<Url, DocState>,
    rules: Arc<RwLock<Vec<(Rule, RuleConfig)>>>,
    project_root: Arc<RwLock<Option<PathBuf>>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
            rules: Arc::new(RwLock::new(Vec::new())),
            project_root: Arc::new(RwLock::new(None)),
        }
    }

    async fn load_rules(&self, root: &PathBuf) {
        let mut rules = Vec::new();

        if let Some((project_root, config)) = ProjectConfig::discover(root) {
            tracing::info!("loading rules from {}", project_root.display());
            for dir in &config.rule_dirs {
                let full = project_root.join(dir);
                if let Ok(entries) = std::fs::read_dir(&full) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "json") {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                match forma_json::from_str::<RuleConfig>(&content) {
                                    Ok(config) => {
                                        if let Some(lang) = SupportLang::from_str(&config.language) {
                                            let ctx = CompileContext {
                                                compile_pattern: |p: &str| {
                                                    let pat = TsPattern::new(p, &lang, lang.ts_language())
                                                        .map_err(|e| e.to_string())?;
                                                    let kinds = pat.potential_kinds();
                                                    Ok((pat.node, kinds))
                                                },
                                                resolve_kind: |k: &str| lang.kind_to_id(k),
                                            };
                                            match compile_rule(&config.rule, &ctx) {
                                                Ok(rule) => rules.push((rule, config)),
                                                Err(e) => tracing::warn!("{}: {e}", path.display()),
                                            }
                                        }
                                    }
                                    Err(e) => tracing::warn!("{}: {e}", path.display()),
                                }
                            }
                        }
                    }
                }
            }
        }

        tracing::info!("loaded {} rules", rules.len());
        *self.rules.write().await = rules;
    }

    async fn diagnose(&self, uri: &Url) {
        let Some(doc) = self.documents.get(uri) else { return };
        let Some(lang) = doc.lang else { return };

        let parsed = match StrDoc::new(&doc.content, lang, lang.ts_language()) {
            Ok(d) => d,
            Err(_) => return,
        };
        let root = Root::new(parsed);
        let rules = self.rules.read().await;

        let mut diagnostics = Vec::new();

        for (rule, config) in rules.iter() {
            if SupportLang::from_str(&config.language) != Some(lang) {
                continue;
            }
            for node in root.root().dfs() {
                if let Some(m) = rule.match_node(node) {
                    let n = m.node();
                    let start = n.start_pos();
                    let end = n.end_pos();

                    // Check for suppression comments.
                    if is_suppressed(&doc.content, start.line, &config.id) {
                        continue;
                    }

                    let severity = match config.severity.unwrap_or_default() {
                        Severity::Error => DiagnosticSeverity::ERROR,
                        Severity::Warning => DiagnosticSeverity::WARNING,
                        Severity::Info => DiagnosticSeverity::INFORMATION,
                        Severity::Hint => DiagnosticSeverity::HINT,
                    };

                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position {
                                line: start.line as u32,
                                character: start.column as u32,
                            },
                            end: Position {
                                line: end.line as u32,
                                character: end.column as u32,
                            },
                        },
                        severity: Some(severity),
                        code: Some(NumberOrString::String(config.id.clone())),
                        source: Some("axe".into()),
                        message: config.message.clone().unwrap_or_else(|| config.id.clone()),
                        ..Default::default()
                    });
                }
            }
        }

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, Some(doc.version))
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Store workspace root.
        if let Some(root_uri) = params.root_uri {
            if let Ok(path) = root_uri.to_file_path() {
                *self.project_root.write().await = Some(path.clone());
                self.load_rules(&path).await;
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        tracing::info!("axe LSP initialized");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let lang_id = &params.text_document.language_id;
        let lang = SupportLang::from_str(lang_id).or_else(|| {
            uri.to_file_path().ok().and_then(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .and_then(SupportLang::from_extension)
            })
        });

        self.documents.insert(
            uri.clone(),
            DocState {
                version: params.text_document.version,
                content: params.text_document.text,
                lang,
            },
        );

        self.diagnose(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(mut doc) = self.documents.get_mut(&uri) {
            // Full sync — replace entire content.
            if let Some(change) = params.content_changes.into_iter().last() {
                doc.content = change.text;
                doc.version = params.text_document.version;
            }
        }
        self.diagnose(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents.remove(&params.text_document.uri);
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let Some(doc) = self.documents.get(&uri) else {
            return Ok(None);
        };
        let Some(lang) = doc.lang else {
            return Ok(None);
        };

        let rules = self.rules.read().await;
        let mut actions = Vec::new();

        // For each diagnostic in the request range, check if the rule has a fix.
        for diag in &params.context.diagnostics {
            if diag.source.as_deref() != Some("axe") {
                continue;
            }
            let rule_id = match &diag.code {
                Some(NumberOrString::String(id)) => id.as_str(),
                _ => continue,
            };

            // Find the rule with this ID.
            for (rule, config) in rules.iter() {
                if config.id != rule_id {
                    continue;
                }
                if let Some(ref fix_template) = config.fix {
                    let parsed = match StrDoc::new(&doc.content, lang, lang.ts_language()) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    let root = Root::new(parsed);

                    for node in root.root().dfs() {
                        let start = node.start_pos();
                        if start.line as u32 == diag.range.start.line
                            && start.column as u32 == diag.range.start.character
                        {
                            if let Some(m) = rule.match_node(node) {
                                let replacement =
                                    axe_core::replacer::apply_template(fix_template, '$', m.env());
                                let edit = TextEdit {
                                    range: diag.range,
                                    new_text: replacement,
                                };
                                let mut changes = HashMap::new();
                                changes.insert(uri.clone(), vec![edit]);

                                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                                    title: format!("Fix: {} (axe)", config.id),
                                    kind: Some(CodeActionKind::QUICKFIX),
                                    diagnostics: Some(vec![diag.clone()]),
                                    edit: Some(WorkspaceEdit {
                                        changes: Some(changes),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }));
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(if actions.is_empty() { None } else { Some(actions) })
    }
}

/// Check if a hit at the given 0-indexed line is suppressed by a comment on the line above.
fn is_suppressed(src: &str, line: usize, rule_id: &str) -> bool {
    if line == 0 {
        return false;
    }
    let lines: Vec<&str> = src.lines().collect();
    let prev_line = if line <= lines.len() {
        lines[line - 1].trim()
    } else {
        return false;
    };

    for prefix in ["//", "#", "--", "/*", "<!--"] {
        if let Some(rest) = prev_line.strip_prefix(prefix) {
            let rest = rest.trim();
            let rest = rest.trim_end_matches("*/").trim_end_matches("-->").trim();

            if rest == "axe-ignore" || rest == "axe-ignore-next-line" {
                return true;
            }
            if let Some(ids) = rest
                .strip_prefix("axe-ignore ")
                .or_else(|| rest.strip_prefix("axe-ignore-next-line "))
            {
                return ids.split(',').map(|s| s.trim()).any(|id| id == rule_id);
            }
        }
    }
    false
}

/// Start the LSP server on stdio.
pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
