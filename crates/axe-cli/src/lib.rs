//! axe CLI — AST eXpression Engine
//!
//! Structural code search, lint, and rewriting powered by tree-sitter.
//! SIF-native output with JSON as a secondary format.
//!
//! ## Commands
//!
//! - `axe run` — One-shot pattern search/replace
//! - `axe scan` — Config-driven multi-rule scanning
//! - `axe test` — Rule testing and snapshot validation
//! - `axe new` — Scaffold rules, projects, tests
//! - `axe lsp` — Language Server Protocol mode
//! - `axe debug` — Interactive REPL for rule development
//! - `axe completions` — Shell completion generation

mod commands;
mod output;

use clap::Parser;
use std::process::ExitCode;

/// axe — AST eXpression Engine
///
/// Structural code search, lint, and rewriting.
#[derive(Parser, Debug)]
#[command(name = "axe", version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output format
    #[arg(long, global = true, default_value = "sif")]
    format: output::OutputFormat,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Search for a pattern and optionally rewrite matches
    Run(commands::run::RunArgs),
    /// Scan with configured rules
    Scan(commands::scan::ScanArgs),
    /// Test rules against snapshots
    Test(commands::test::TestArgs),
    /// Generate a new rule, project, or test
    New(commands::new::NewArgs),
    /// Start the language server
    Lsp(commands::lsp::LspArgs),
    /// Generate shell completions
    Completions(commands::completions::CompletionsArgs),
}

/// Entry point for the CLI.
pub fn run() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => commands::run::execute(args, cli.format),
        Commands::Scan(args) => commands::scan::execute(args, cli.format),
        Commands::Test(args) => commands::test::execute(args),
        Commands::New(args) => commands::new::execute(args),
        Commands::Lsp(args) => commands::lsp::execute(args),
        Commands::Completions(args) => commands::completions::execute(args),
    }
}
