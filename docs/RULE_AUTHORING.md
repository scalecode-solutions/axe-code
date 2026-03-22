# Writing Rules for axe

This guide walks through creating, testing, and deploying custom rules.

## Creating a Rule

The fastest way to start:

```bash
axe new rule no-eval --lang javascript > rules/no-eval.json
```

This generates a scaffold. Edit the `pattern`, `message`, and `tests` fields.

## Rule Structure

Every rule needs three things: an **id**, a **language**, and a **rule**.

```json
{
  "id": "no-eval",
  "language": "javascript",
  "rule": {
    "pattern": "eval($CODE)"
  },
  "severity": "Error",
  "message": "eval() is a security risk — use safer alternatives"
}
```

## Patterns

Patterns are code snippets where `$VARIABLES` match any AST node.

### Single capture: `$VAR`

```
console.log($MSG)      matches: console.log("hello"), console.log(x + y)
const $A = $B          matches: const x = 42, const name = getName()
if ($COND) $THEN       matches: if (true) return, if (x > 0) { ... }
```

### Multi-capture: `$$$VAR`

Captures zero or more nodes (like spread/rest):

```
fn($$$ARGS)            matches: fn(), fn(a), fn(a, b, c)
{ $$$BODY }            matches: {}, { x; }, { x; y; z; }
[$$$ITEMS]             matches: [], [1], [1, 2, 3]
```

### Anonymous: `$_`

Matches any node without capturing (useful in composite rules):

```
$_($A)                 matches any function call with one argument
```

## Composite Rules

### `all` — every sub-rule must match

Find `console.log` that's NOT inside a catch block:

```json
{
  "rule": {
    "all": [
      { "pattern": "console.log($A)" },
      { "not": { "inside": { "rule": { "kind": "catch_clause" } } } }
    ]
  }
}
```

### `any` — at least one sub-rule must match

Find any console method:

```json
{
  "rule": {
    "any": [
      { "pattern": "console.log($A)" },
      { "pattern": "console.warn($A)" },
      { "pattern": "console.debug($A)" }
    ]
  }
}
```

### `not` — negate a rule

Match everything except error logging:

```json
{
  "rule": {
    "all": [
      { "pattern": "console.$METHOD($A)" },
      { "not": { "pattern": "console.error($A)" } }
    ]
  }
}
```

### `inside` — match within an ancestor

Find returns inside async functions:

```json
{
  "rule": {
    "pattern": "return $VAL",
    "inside": {
      "rule": { "kind": "async_function" }
    }
  }
}
```

### `has` — match with a descendant

Find functions that contain a TODO comment:

```json
{
  "rule": {
    "kind": "function_declaration",
    "has": {
      "rule": { "regex": "TODO|FIXME" }
    }
  }
}
```

## Fixes

Add a `fix` field with a replacement template. The template uses the same `$VAR` syntax:

```json
{
  "id": "prefer-const",
  "language": "javascript",
  "rule": { "pattern": "var $A = $B" },
  "fix": "const $A = $B",
  "message": "Use const instead of var"
}
```

When running `axe scan --apply`, the fix is applied automatically.

## Testing Rules

Every rule should have test cases:

```json
{
  "tests": {
    "valid": [
      "const x = 42",
      "let y = compute()"
    ],
    "invalid": [
      "var x = 42",
      "var name = 'alice'"
    ]
  }
}
```

- **valid**: code that should NOT trigger the rule (correct code)
- **invalid**: code that SHOULD trigger the rule (code with the problem)

Run tests:

```bash
axe test -r rules/
```

## Suppressing Rules

Add a comment on the line above to suppress:

```javascript
// axe-ignore
var x = 42;  // This line is not flagged

// axe-ignore no-var
var y = 99;  // Only no-var is suppressed

// axe-ignore no-var, no-eval
var z = eval("1+1");  // Both rules suppressed
```

## Project Setup

1. Create `axeconfig.json` at your project root:

```bash
axe new config > axeconfig.json
```

2. Create a `rules/` directory:

```bash
mkdir rules
```

3. Add rules:

```bash
axe new rule no-console-log --lang js > rules/no-console-log.json
# Edit the rule...
```

4. Run:

```bash
axe scan src/         # Scan
axe test              # Test rules
axe scan --apply src/ # Auto-fix
```

## CI Integration

### GitHub Actions

```yaml
- name: Run axe
  run: axe scan --format github src/
```

### SARIF upload

```yaml
- name: Run axe
  run: axe scan --format sarif src/ > axe-results.sarif
- name: Upload SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: axe-results.sarif
```

## Tips

1. **Start simple.** A single-pattern rule catches most issues. Add composite rules only when needed.

2. **Use `axe run` to prototype.** Before writing a rule file, test your pattern interactively:
   ```bash
   axe run -p 'your_pattern' -l lang file.ext
   ```

3. **Check captures.** The output shows `$VAR=value` so you can verify what's captured.

4. **Test both sides.** Every `invalid` test should have a corresponding `valid` test that's almost the same but correct.

5. **Use `--format json`** to pipe results into other tools:
   ```bash
   axe scan --format json src/ | jq '.rule'
   ```
