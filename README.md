# axe

**AST eXpression Engine** — structural code search, lint, and rewriting powered by tree-sitter.

axe finds and transforms code by matching against the AST (Abstract Syntax Tree), not text patterns. Write a pattern like `console.log($A)` and axe finds every call to `console.log` regardless of formatting, comments, or whitespace — and can rewrite them all in one command.

## Quick Start

```bash
# Search for a pattern
axe run -p 'console.log($A)' -l javascript src/

# Rewrite matches (preview)
axe run -p 'console.log($A)' -r 'logger.info($A)' -l javascript src/

# Rewrite matches (apply to files)
axe run -p 'console.log($A)' -r 'logger.info($A)' --apply -l javascript src/

# Scan with rules
axe scan -r rules/ src/

# Test your rules
axe test -r rules/
```

## Features

- **Structural matching** — patterns match AST structure, not text. `$A` captures any single node, `$$$ARGS` captures zero or more.
- **26 languages** — Bash, C, C++, C#, CSS, Elixir, Go, Haskell, HCL/Terraform, HTML, Java, JavaScript, JSON, Kotlin, Lua, Nix, PHP, Python, Ruby, Rust, Scala, Solidity, Swift, TypeScript, TSX, YAML.
- **Code rewriting** — template-based rewrites with `$VAR` substitution and indentation preservation.
- **Rule-based scanning** — JSON rule files with severity levels, test cases, and auto-fix templates.
- **Suppression comments** — `// axe-ignore` or `// axe-ignore rule-id` to silence specific diagnostics.
- **LSP server** — diagnostics and quick-fix code actions in your editor.
- **SIF-native output** — structured, typed, schema-first output. JSON, SARIF, and GitHub Actions formats also supported.
- **Parallel scanning** — multi-threaded file walking for large codebases.
- **104 tests** including adversarial testing (Furling Test: Chevrons 1, 3, 6).

## Installation

```bash
# From source
git clone https://github.com/scalecode-solutions/axe-code
cd axe-code
cargo install --path crates/axe-cli
```

## Patterns

axe patterns use `$` for meta-variables:

| Pattern | Matches |
|---------|---------|
| `console.log($A)` | Any `console.log` call with one argument |
| `const $A = $B` | Any `const` declaration |
| `fn $NAME($$$ARGS) { $$$BODY }` | Any function with any arguments and body |
| `if ($COND) { $$$THEN }` | Any `if` statement |
| `$A + $B` | Any addition expression |

- `$VAR` — captures a single named AST node
- `$$$VAR` — captures zero or more nodes (ellipsis)
- `$_` — matches any node without capturing

## Commands

### `axe run` — search and rewrite

```bash
# Search
axe run -p 'pattern' -l language [paths...]

# Search with JSON output
axe run -p 'pattern' -l language --format json [paths...]

# Rewrite preview
axe run -p 'pattern' -r 'replacement' -l language [paths...]

# Rewrite and apply
axe run -p 'pattern' -r 'replacement' --apply -l language [paths...]

# Limit results
axe run -p 'pattern' -l language --max-results 10 [paths...]
```

### `axe scan` — rule-based scanning

```bash
# Scan with rule files
axe scan -r rules/no-console-log.json src/

# Scan with a rule directory
axe scan -r rules/ src/

# Auto-discover rules from axeconfig.json
axe scan src/

# Auto-fix
axe scan --apply src/

# Filter by severity
axe scan --severity error src/

# SARIF output for CI
axe scan --format sarif src/ > results.sarif

# GitHub Actions annotations
axe scan --format github src/
```

### `axe test` — validate rules

```bash
# Test all rules in a directory
axe test -r rules/

# Auto-discover from axeconfig.json
axe test

# Filter by rule ID
axe test --filter no-console
```

### `axe new` — scaffolding

```bash
# Generate a rule scaffold
axe new rule no-eval --lang javascript > rules/no-eval.json

# Generate a project config
axe new config > axeconfig.json
```

### `axe lsp` — language server

```bash
# Start LSP on stdio (for editor integration)
axe lsp
```

### `axe completions` — shell completions

```bash
# Generate completions
axe completions bash >> ~/.bashrc
axe completions zsh >> ~/.zshrc
axe completions fish > ~/.config/fish/completions/axe.fish
```

## Rule Format

Rules are JSON files:

```json
{
  "id": "no-console-log",
  "language": "javascript",
  "rule": {
    "pattern": "console.log($A)"
  },
  "severity": "Warning",
  "message": "Avoid console.log in production code",
  "fix": "logger.info($A)",
  "tests": {
    "valid": [
      "logger.info('ok')",
      "console.error('err')"
    ],
    "invalid": [
      "console.log('hello')",
      "console.log(x + y)"
    ]
  }
}
```

### Rule fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique rule identifier |
| `language` | Yes | Target language |
| `rule` | Yes | The matching rule (see below) |
| `severity` | No | `Hint`, `Info`, `Warning`, `Error` (default: Warning) |
| `message` | No | Human-readable diagnostic message |
| `fix` | No | Rewrite template for auto-fix |
| `tests` | No | Test cases for `axe test` |
| `url` | No | Link to documentation |

### Rule types

```json
// Pattern matching
{ "pattern": "console.log($A)" }

// Node kind matching
{ "kind": "function_declaration" }

// Regex on node text
{ "regex": "TODO|FIXME" }

// Composite: all must match
{ "all": [
    { "pattern": "console.log($A)" },
    { "not": { "inside": { "kind": "catch_clause" } } }
  ]
}

// Any must match
{ "any": [
    { "pattern": "console.log($A)" },
    { "pattern": "console.warn($A)" }
  ]
}

// Negation
{ "not": { "pattern": "console.error($A)" } }

// Relational: inside an ancestor
{ "pattern": "$A", "inside": { "rule": { "kind": "function_declaration" } } }

// Relational: has a descendant
{ "pattern": "$A", "has": { "rule": { "kind": "return_statement" } } }
```

### Suppression comments

Suppress diagnostics with comments on the line above:

```javascript
// axe-ignore
console.log("this line is ignored");

// axe-ignore no-console-log
console.log("only no-console-log is ignored here");

// axe-ignore no-var, no-console-log
var x = console.log("multiple rules ignored");
```

Works with all comment styles: `//`, `#`, `--`, `/*`, `<!--`.

## Project Config

Create `axeconfig.json` at your project root:

```json
{
  "rule_dirs": ["rules", ".axe/rules"],
  "rules": ["extra-rules/special.json"]
}
```

Both `axe scan` and `axe test` auto-discover this config by walking up from the current directory. No `--rule` flag needed.

## Output Formats

| Format | Flag | Use case |
|--------|------|----------|
| SIF | `--format sif` (default) | Structured, typed, pipe-friendly |
| JSON | `--format json` | One JSON object per line |
| SARIF | `--format sarif` | IDE integration, CI upload |
| GitHub | `--format github` | GitHub Actions annotations |
| Plain | `--format color` | Human-readable terminal output |

### SIF output example

```
#!sif v1 origin=axe/scan
#schema file:str:311 line:uint:341 col:uint:341 rule:str severity:str message:str match:str
src/app.js	1	1	no-var	error	Use const or let	var x = 42;
src/app.js	3	1	no-console-log	warning	Avoid console.log	console.log(x);
```

## Architecture

axe is a ground-up rewrite inspired by [ast-grep](https://github.com/ast-grep/ast-grep) with key improvements:

- **Zero unsafe code** — `Arc`-based memory safety for FFI, no lifetime transmutes
- **[forma](https://crates.io/crates/forma-io)** for serialization (not serde)
- **[SIF](https://github.com/scalecode-solutions/sif-parser)** as the primary output format
- **AHash** for all hash maps (faster than std HashMap)
- **Parallel file walking** with crossbeam channels
- **Tree-sitter cursor traversal** for relational operators (reduced stack usage)
- **TAB-aware indentation** in rewrites
- **Recursion depth limits** on rule compilation (prevents stack overflow)

### Workspace structure

```
crates/
  axe-core/         Core matching engine (encoding-agnostic, tree-sitter-agnostic)
  axe-tree-sitter/  Tree-sitter integration (StrDoc, TsNode, OwnedRoot, TsPattern)
  axe-config/       Rule deserialization, compilation, CombinedScan
  axe-language/     26 built-in language definitions
  axe-dynamic/      Dynamic language loading (runtime .so/.dylib)
  axe-cli/          CLI binary and commands
  axe-lsp/          Language Server Protocol implementation
```

## License

MIT
