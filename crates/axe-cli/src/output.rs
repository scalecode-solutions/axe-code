//! Output format handling — SIF-native with JSON as secondary.

/// Supported output formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Structured Interchange Format (default)
    Sif,
    /// JSON (one object per line)
    Json,
    /// Ripgrep-compatible JSON
    Rg,
    /// GitHub Actions annotations
    Github,
    /// SARIF for IDE integration
    Sarif,
    /// Colored terminal output (auto-detected for TTY)
    Color,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Sif
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Sif => write!(f, "sif"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Rg => write!(f, "rg"),
            OutputFormat::Github => write!(f, "github"),
            OutputFormat::Sarif => write!(f, "sarif"),
            OutputFormat::Color => write!(f, "color"),
        }
    }
}

/// SIF origin header for axe output.
pub fn sif_origin(command: &str) -> String {
    format!("axe/{command}")
}
