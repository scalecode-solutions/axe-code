//! Built-in language registry for axe.
//!
//! Provides [`SupportLang`] — an enum of all built-in languages with their
//! tree-sitter grammars, meta-variable conventions, and file extension mappings.

use std::borrow::Cow;

use axe_core::language::Language;

// ---------------------------------------------------------------------------
// Language macro — generates Language impl for simple languages
// ---------------------------------------------------------------------------

/// Generate a Language impl for languages where `$` works as identifier start.
/// These languages use `$` -> `µ` substitution (meta_var_char = '$', expando = 'µ').
macro_rules! impl_lang {
    ($name:ident, $ts_fn:expr) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $name;

        impl $name {
            pub fn ts_language() -> tree_sitter::Language {
                $ts_fn()
            }
        }

        impl Language for $name {
            fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
                // $ is valid in the grammar — replace $VAR with µVAR for parsing
                if query.contains('$') {
                    Cow::Owned(query.replace('$', "µ"))
                } else {
                    Cow::Borrowed(query)
                }
            }

            fn kind_to_id(&self, kind: &str) -> Option<u16> {
                let lang = Self::ts_language();
                let id = lang.id_for_node_kind(kind, true);
                if id == 0 {
                    // Try unnamed
                    let id = lang.id_for_node_kind(kind, false);
                    if id == 0 { None } else { Some(id) }
                } else {
                    Some(id)
                }
            }

            fn id_to_kind(&self, id: u16) -> &str {
                // tree-sitter returns a static str
                let lang = Self::ts_language();
                lang.node_kind_for_id(id).unwrap_or("UNKNOWN")
            }

            fn field_to_id(&self, field: &str) -> Option<u16> {
                Self::ts_language().field_id_for_name(field).map(|id| id.get())
            }

            fn kind_count(&self) -> usize {
                Self::ts_language().node_kind_count()
            }
        }
    };
}

/// Generate a Language impl for languages where `$` does NOT work as identifier
/// start. Each such language has a specific expando char used internally.
macro_rules! impl_lang_expando {
    ($name:ident, $ts_fn:expr, $expando:expr) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $name;

        impl $name {
            pub fn ts_language() -> tree_sitter::Language {
                $ts_fn()
            }
        }

        impl Language for $name {
            fn meta_var_char(&self) -> char {
                '$'
            }

            fn expando_char(&self) -> char {
                $expando
            }

            fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
                let ec = $expando;
                if query.contains('$') {
                    Cow::Owned(query.replace('$', &ec.to_string()))
                } else {
                    Cow::Borrowed(query)
                }
            }

            fn kind_to_id(&self, kind: &str) -> Option<u16> {
                let lang = Self::ts_language();
                let id = lang.id_for_node_kind(kind, true);
                if id == 0 {
                    let id = lang.id_for_node_kind(kind, false);
                    if id == 0 { None } else { Some(id) }
                } else {
                    Some(id)
                }
            }

            fn id_to_kind(&self, id: u16) -> &str {
                Self::ts_language().node_kind_for_id(id).unwrap_or("UNKNOWN")
            }

            fn field_to_id(&self, field: &str) -> Option<u16> {
                Self::ts_language().field_id_for_name(field).map(|id| id.get())
            }

            fn kind_count(&self) -> usize {
                Self::ts_language().node_kind_count()
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Built-in languages — impl_lang! ($ works as identifier start)
// ---------------------------------------------------------------------------

impl_lang!(Bash, || tree_sitter_bash::LANGUAGE.into());
impl_lang!(Java, || tree_sitter_java::LANGUAGE.into());
impl_lang!(JavaScript, || tree_sitter_javascript::LANGUAGE.into());
impl_lang!(Json, || tree_sitter_json::LANGUAGE.into());
impl_lang!(Lua, || tree_sitter_lua::LANGUAGE.into());
impl_lang!(Scala, || tree_sitter_scala::LANGUAGE.into());
impl_lang!(Solidity, || tree_sitter_solidity::LANGUAGE.into());
impl_lang!(Tsx, || tree_sitter_typescript::LANGUAGE_TSX.into());
impl_lang!(TypeScript, || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
impl_lang!(Yaml, || tree_sitter_yaml::LANGUAGE.into());

// ---------------------------------------------------------------------------
// Built-in languages — impl_lang_expando! ($ does NOT work)
// ---------------------------------------------------------------------------

impl_lang_expando!(C, || tree_sitter_c::LANGUAGE.into(), '\u{10000}');
impl_lang_expando!(Cpp, || tree_sitter_cpp::LANGUAGE.into(), '\u{10000}');
impl_lang_expando!(CSharp, || tree_sitter_c_sharp::LANGUAGE.into(), 'µ');
impl_lang_expando!(Css, || tree_sitter_css::LANGUAGE.into(), '_');
impl_lang_expando!(Elixir, || tree_sitter_elixir::LANGUAGE.into(), 'µ');
impl_lang_expando!(Go, || tree_sitter_go::LANGUAGE.into(), 'µ');
impl_lang_expando!(Haskell, || tree_sitter_haskell::LANGUAGE.into(), 'µ');
impl_lang_expando!(Hcl, || tree_sitter_hcl::LANGUAGE.into(), 'µ');
impl_lang_expando!(Html, || tree_sitter_html::LANGUAGE.into(), 'z');
impl_lang_expando!(Kotlin, || tree_sitter_kotlin::LANGUAGE.into(), 'µ');
impl_lang_expando!(Nix, || tree_sitter_nix::LANGUAGE.into(), '_');
impl_lang_expando!(Php, || tree_sitter_php::LANGUAGE_PHP_ONLY.into(), 'µ');
impl_lang_expando!(Python, || tree_sitter_python::LANGUAGE.into(), 'µ');
impl_lang_expando!(Ruby, || tree_sitter_ruby::LANGUAGE.into(), 'µ');
impl_lang_expando!(Rust, || tree_sitter_rust::LANGUAGE.into(), 'µ');
impl_lang_expando!(Swift, || tree_sitter_swift::LANGUAGE.into(), 'µ');

// ---------------------------------------------------------------------------
// SupportLang enum
// ---------------------------------------------------------------------------

/// All supported built-in languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SupportLang {
    Bash,
    C,
    Cpp,
    CSharp,
    Css,
    Elixir,
    Go,
    Haskell,
    Hcl,
    Html,
    Java,
    JavaScript,
    Json,
    Kotlin,
    Lua,
    Nix,
    Php,
    Python,
    Ruby,
    Rust,
    Scala,
    Solidity,
    Swift,
    TypeScript,
    Tsx,
    Yaml,
}

impl SupportLang {
    /// Get the tree-sitter Language for this language.
    pub fn ts_language(&self) -> tree_sitter::Language {
        match self {
            Self::Bash => Bash::ts_language(),
            Self::C => C::ts_language(),
            Self::Cpp => Cpp::ts_language(),
            Self::CSharp => CSharp::ts_language(),
            Self::Css => Css::ts_language(),
            Self::Elixir => Elixir::ts_language(),
            Self::Go => Go::ts_language(),
            Self::Haskell => Haskell::ts_language(),
            Self::Hcl => Hcl::ts_language(),
            Self::Html => Html::ts_language(),
            Self::Java => Java::ts_language(),
            Self::JavaScript => JavaScript::ts_language(),
            Self::Json => Json::ts_language(),
            Self::Kotlin => Kotlin::ts_language(),
            Self::Lua => Lua::ts_language(),
            Self::Nix => Nix::ts_language(),
            Self::Php => Php::ts_language(),
            Self::Python => Python::ts_language(),
            Self::Ruby => Ruby::ts_language(),
            Self::Rust => Rust::ts_language(),
            Self::Scala => Scala::ts_language(),
            Self::Solidity => Solidity::ts_language(),
            Self::Swift => Swift::ts_language(),
            Self::TypeScript => TypeScript::ts_language(),
            Self::Tsx => Tsx::ts_language(),
            Self::Yaml => Yaml::ts_language(),
        }
    }

    /// File extensions associated with this language.
    pub fn file_types(&self) -> &'static [&'static str] {
        match self {
            Self::Bash => &["sh", "bash", "zsh"],
            Self::C => &["c", "h"],
            Self::Cpp => &["cpp", "cc", "cxx", "hpp", "hxx", "h"],
            Self::CSharp => &["cs"],
            Self::Css => &["css"],
            Self::Elixir => &["ex", "exs"],
            Self::Go => &["go"],
            Self::Haskell => &["hs", "lhs"],
            Self::Hcl => &["hcl", "tf", "tfvars"],
            Self::Html => &["html", "htm"],
            Self::Java => &["java"],
            Self::JavaScript => &["js", "mjs", "cjs"],
            Self::Json => &["json"],
            Self::Kotlin => &["kt", "kts"],
            Self::Lua => &["lua"],
            Self::Nix => &["nix"],
            Self::Php => &["php"],
            Self::Python => &["py", "pyi"],
            Self::Ruby => &["rb"],
            Self::Rust => &["rs"],
            Self::Scala => &["scala", "sc"],
            Self::Solidity => &["sol"],
            Self::Swift => &["swift"],
            Self::TypeScript => &["ts", "mts", "cts"],
            Self::Tsx => &["tsx"],
            Self::Yaml => &["yml", "yaml"],
        }
    }

    /// Parse a language name string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "bash" | "sh" => Some(Self::Bash),
            "c" => Some(Self::C),
            "cpp" | "c++" | "cc" => Some(Self::Cpp),
            "csharp" | "c#" | "cs" => Some(Self::CSharp),
            "css" => Some(Self::Css),
            "elixir" | "ex" => Some(Self::Elixir),
            "go" | "golang" => Some(Self::Go),
            "haskell" | "hs" => Some(Self::Haskell),
            "hcl" | "terraform" | "tf" => Some(Self::Hcl),
            "html" => Some(Self::Html),
            "java" => Some(Self::Java),
            "javascript" | "js" => Some(Self::JavaScript),
            "json" => Some(Self::Json),
            "kotlin" | "kt" => Some(Self::Kotlin),
            "lua" => Some(Self::Lua),
            "nix" => Some(Self::Nix),
            "php" => Some(Self::Php),
            "python" | "py" => Some(Self::Python),
            "ruby" | "rb" => Some(Self::Ruby),
            "rust" | "rs" => Some(Self::Rust),
            "scala" => Some(Self::Scala),
            "solidity" | "sol" => Some(Self::Solidity),
            "swift" => Some(Self::Swift),
            "typescript" | "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "yaml" | "yml" => Some(Self::Yaml),
            _ => None,
        }
    }

    /// Detect language from a file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.strip_prefix('.').unwrap_or(ext);
        // Check all languages' file_types
        for lang in Self::all() {
            if lang.file_types().contains(&ext) {
                return Some(*lang);
            }
        }
        None
    }

    /// All supported languages.
    pub fn all() -> &'static [Self] {
        &[
            Self::Bash,
            Self::C,
            Self::Cpp,
            Self::CSharp,
            Self::Css,
            Self::Elixir,
            Self::Go,
            Self::Haskell,
            Self::Hcl,
            Self::Html,
            Self::Java,
            Self::JavaScript,
            Self::Json,
            Self::Kotlin,
            Self::Lua,
            Self::Nix,
            Self::Php,
            Self::Python,
            Self::Ruby,
            Self::Rust,
            Self::Scala,
            Self::Solidity,
            Self::Swift,
            Self::TypeScript,
            Self::Tsx,
            Self::Yaml,
        ]
    }
}

// ---------------------------------------------------------------------------
// Language trait impl for SupportLang
// ---------------------------------------------------------------------------

impl Language for SupportLang {
    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        // Users always write $VAR. We replace $ with the language's expando char.
        let ec = self.expando_char();
        if ec == '$' {
            // No replacement needed (shouldn't happen with current languages).
            Cow::Borrowed(query)
        } else if query.contains('$') {
            Cow::Owned(query.replace('$', &ec.to_string()))
        } else {
            Cow::Borrowed(query)
        }
    }

    fn meta_var_char(&self) -> char {
        // Users always write $VAR for all languages.
        '$'
    }

    fn expando_char(&self) -> char {
        match self {
            Self::C | Self::Cpp => '\u{10000}',
            Self::Css | Self::Nix => '_',
            Self::Html => 'z',
            // All others use µ
            _ => 'µ',
        }
    }

    fn kind_to_id(&self, kind: &str) -> Option<u16> {
        let lang = self.ts_language();
        let id = lang.id_for_node_kind(kind, true);
        if id == 0 {
            let id = lang.id_for_node_kind(kind, false);
            if id == 0 { None } else { Some(id) }
        } else {
            Some(id)
        }
    }

    fn id_to_kind(&self, id: u16) -> &str {
        self.ts_language().node_kind_for_id(id).unwrap_or("UNKNOWN")
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        self.ts_language().field_id_for_name(field).map(|id| id.get())
    }

    fn kind_count(&self) -> usize {
        self.ts_language().node_kind_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_variants() {
        assert_eq!(SupportLang::from_str("rust"), Some(SupportLang::Rust));
        assert_eq!(SupportLang::from_str("JavaScript"), Some(SupportLang::JavaScript));
        assert_eq!(SupportLang::from_str("c++"), Some(SupportLang::Cpp));
        assert_eq!(SupportLang::from_str("golang"), Some(SupportLang::Go));
        assert_eq!(SupportLang::from_str("kotlin"), Some(SupportLang::Kotlin));
        assert_eq!(SupportLang::from_str("elixir"), Some(SupportLang::Elixir));
        assert_eq!(SupportLang::from_str("hcl"), Some(SupportLang::Hcl));
        assert_eq!(SupportLang::from_str("nix"), Some(SupportLang::Nix));
        assert_eq!(SupportLang::from_str("php"), Some(SupportLang::Php));
        assert_eq!(SupportLang::from_str("scala"), Some(SupportLang::Scala));
        assert_eq!(SupportLang::from_str("solidity"), Some(SupportLang::Solidity));
        assert_eq!(SupportLang::from_str("haskell"), Some(SupportLang::Haskell));
        assert_eq!(SupportLang::from_str("nope"), None);
    }

    #[test]
    fn from_extension_variants() {
        assert_eq!(SupportLang::from_extension("rs"), Some(SupportLang::Rust));
        assert_eq!(SupportLang::from_extension(".py"), Some(SupportLang::Python));
        assert_eq!(SupportLang::from_extension("tsx"), Some(SupportLang::Tsx));
        assert_eq!(SupportLang::from_extension("kt"), Some(SupportLang::Kotlin));
        assert_eq!(SupportLang::from_extension("ex"), Some(SupportLang::Elixir));
        assert_eq!(SupportLang::from_extension("sol"), Some(SupportLang::Solidity));
        assert_eq!(SupportLang::from_extension("nix"), Some(SupportLang::Nix));
        assert_eq!(SupportLang::from_extension("tf"), Some(SupportLang::Hcl));
        assert_eq!(SupportLang::from_extension("xyz"), None);
    }

    #[test]
    fn all_languages_have_file_types() {
        for lang in SupportLang::all() {
            assert!(!lang.file_types().is_empty(), "{lang:?} has no file types");
        }
    }

    #[test]
    fn expando_chars_correct() {
        use axe_core::language::Language;
        assert_eq!(SupportLang::C.expando_char(), '\u{10000}');
        assert_eq!(SupportLang::Cpp.expando_char(), '\u{10000}');
        assert_eq!(SupportLang::Css.expando_char(), '_');
        assert_eq!(SupportLang::Nix.expando_char(), '_');
        assert_eq!(SupportLang::Html.expando_char(), 'z');
        assert_eq!(SupportLang::Rust.expando_char(), 'µ');
        assert_eq!(SupportLang::Python.expando_char(), 'µ');
        assert_eq!(SupportLang::Go.expando_char(), 'µ');
    }

    #[test]
    fn all_languages_count() {
        assert_eq!(SupportLang::all().len(), 26);
    }
}
