//! `axe new` — scaffold rules, projects, tests.

use std::process::ExitCode;

#[derive(clap::Args, Debug)]
pub struct NewArgs {
    /// What to create: rule, config
    #[command(subcommand)]
    pub entity: NewEntity,
}

#[derive(clap::Subcommand, Debug)]
pub enum NewEntity {
    /// Generate a new rule scaffold (JSON)
    Rule {
        /// Rule ID (e.g., no-eval, no-console-log)
        id: String,
        /// Language for the rule (e.g., js, python, rust)
        #[arg(short, long)]
        lang: String,
    },
    /// Generate a starter axeconfig.json
    Config,
}

pub fn execute(args: NewArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    tracing::debug!(?args, "executing new command");

    match args.entity {
        NewEntity::Rule { id, lang } => {
            // Validate the language
            if axe_language::SupportLang::from_str(&lang).is_none() {
                eprintln!("axe new: unknown language '{lang}'");
                eprintln!("supported: bash, c, cpp, csharp, css, elixir, go, haskell, hcl, html, java, javascript, json, kotlin, lua, nix, php, python, ruby, rust, scala, solidity, swift, typescript, tsx, yaml");
                return Ok(ExitCode::from(1));
            }

            let rule_json = format!(
                r#"{{
  "id": "{id}",
  "language": "{lang}",
  "rule": {{ "pattern": "TODO_PATTERN" }},
  "severity": "Warning",
  "message": "TODO: describe what this rule catches",
  "tests": {{
    "valid": ["TODO: code that should pass"],
    "invalid": ["TODO: code that should fail"]
  }}
}}"#
            );
            println!("{rule_json}");
        }
        NewEntity::Config => {
            let config_json = r#"{
  "rule_dirs": ["rules"]
}"#;
            println!("{config_json}");
        }
    }

    Ok(ExitCode::SUCCESS)
}
