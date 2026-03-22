//! Built-in language registry for axe.
//!
//! Provides [`SupportLang`] — an enum of all built-in languages with their
//! tree-sitter grammars, meta-variable conventions, and file extension mappings.

use std::borrow::Cow;

use axe_core::language::Language;

// ---------------------------------------------------------------------------
// Language macro — generates Language impl for simple languages
// ---------------------------------------------------------------------------

/// Generate a Language impl for languages where `$` is a valid identifier char.
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

/// Generate a Language impl for languages where `$` is syntax (e.g., PHP, Bash).
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
                $expando
            }

            fn expando_char(&self) -> char {
                'µ'
            }

            fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
                let mc = self.meta_var_char();
                if query.contains(mc) {
                    Cow::Owned(query.replace(mc, "µ"))
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
// Built-in languages
// ---------------------------------------------------------------------------

impl_lang!(Rust, || tree_sitter_rust::LANGUAGE.into());
impl_lang!(JavaScript, || tree_sitter_javascript::LANGUAGE.into());
impl_lang!(TypeScript, || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
impl_lang!(Tsx, || tree_sitter_typescript::LANGUAGE_TSX.into());
impl_lang!(Python, || tree_sitter_python::LANGUAGE.into());
impl_lang!(Go, || tree_sitter_go::LANGUAGE.into());
impl_lang!(Java, || tree_sitter_java::LANGUAGE.into());
impl_lang!(C, || tree_sitter_c::LANGUAGE.into());
impl_lang!(Cpp, || tree_sitter_cpp::LANGUAGE.into());
impl_lang!(CSharp, || tree_sitter_c_sharp::LANGUAGE.into());
impl_lang!(Css, || tree_sitter_css::LANGUAGE.into());
impl_lang!(Html, || tree_sitter_html::LANGUAGE.into());
impl_lang!(Json, || tree_sitter_json::LANGUAGE.into());
impl_lang!(Ruby, || tree_sitter_ruby::LANGUAGE.into());
impl_lang!(Swift, || tree_sitter_swift::LANGUAGE.into());
// TODO: kotlin and toml use older tree-sitter API, need version-compatible crates
// impl_lang!(Kotlin, || tree_sitter_kotlin::language().into());
impl_lang!(Lua, || tree_sitter_lua::LANGUAGE.into());
impl_lang!(Yaml, || tree_sitter_yaml::LANGUAGE.into());
// impl_lang!(Toml, || tree_sitter_toml::language().into());

// Languages where $ is syntax
impl_lang_expando!(Bash, || tree_sitter_bash::LANGUAGE.into(), '#');

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
    Go,
    Html,
    Java,
    JavaScript,
    Json,
    // Kotlin,  // TODO: needs tree-sitter 0.26 compatible crate
    Lua,
    Python,
    Ruby,
    Rust,
    Swift,
    // Toml,  // TODO: needs tree-sitter 0.26 compatible crate
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
            Self::Go => Go::ts_language(),
            Self::Html => Html::ts_language(),
            Self::Java => Java::ts_language(),
            Self::JavaScript => JavaScript::ts_language(),
            Self::Json => Json::ts_language(),
            // Self::Kotlin => Kotlin::ts_language(),
            Self::Lua => Lua::ts_language(),
            Self::Python => Python::ts_language(),
            Self::Ruby => Ruby::ts_language(),
            Self::Rust => Rust::ts_language(),
            Self::Swift => Swift::ts_language(),
            // Self::Toml => Toml::ts_language(),
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
            Self::Go => &["go"],
            Self::Html => &["html", "htm"],
            Self::Java => &["java"],
            Self::JavaScript => &["js", "mjs", "cjs"],
            Self::Json => &["json"],
            // Self::Kotlin => &["kt", "kts"],
            Self::Lua => &["lua"],
            Self::Python => &["py", "pyi"],
            Self::Ruby => &["rb"],
            Self::Rust => &["rs"],
            Self::Swift => &["swift"],
            // Self::Toml => &["toml"],
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
            "go" | "golang" => Some(Self::Go),
            "html" => Some(Self::Html),
            "java" => Some(Self::Java),
            "javascript" | "js" => Some(Self::JavaScript),
            "json" => Some(Self::Json),
            // "kotlin" | "kt" => Some(Self::Kotlin),
            "lua" => Some(Self::Lua),
            "python" | "py" => Some(Self::Python),
            "ruby" | "rb" => Some(Self::Ruby),
            "rust" | "rs" => Some(Self::Rust),
            "swift" => Some(Self::Swift),
            // "toml" => Some(Self::Toml),
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

    /// Whether this language uses a non-standard meta-var char.
    fn uses_expando(&self) -> bool {
        matches!(self, Self::Bash)
    }

    /// All supported languages.
    pub fn all() -> &'static [Self] {
        &[
            Self::Bash,
            Self::C,
            Self::Cpp,
            Self::CSharp,
            Self::Css,
            Self::Go,
            Self::Html,
            Self::Java,
            Self::JavaScript,
            Self::Json,
            // Self::Kotlin,
            Self::Lua,
            Self::Python,
            Self::Ruby,
            Self::Rust,
            Self::Swift,
            // Self::Toml,
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
        let mc = self.meta_var_char();
        let ec = self.expando_char();
        if mc == ec {
            // Standard language — replace $ with µ.
            if query.contains('$') {
                Cow::Owned(query.replace('$', "µ"))
            } else {
                Cow::Borrowed(query)
            }
        } else {
            // Non-standard (e.g., Bash uses #).
            if query.contains(mc) {
                Cow::Owned(query.replace(mc, &ec.to_string()))
            } else {
                Cow::Borrowed(query)
            }
        }
    }

    fn meta_var_char(&self) -> char {
        if self.uses_expando() {
            '#' // Bash
        } else {
            '$'
        }
    }

    fn expando_char(&self) -> char {
        'µ'
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
        assert_eq!(SupportLang::from_str("nope"), None);
    }

    #[test]
    fn from_extension_variants() {
        assert_eq!(SupportLang::from_extension("rs"), Some(SupportLang::Rust));
        assert_eq!(SupportLang::from_extension(".py"), Some(SupportLang::Python));
        assert_eq!(SupportLang::from_extension("tsx"), Some(SupportLang::Tsx));
        assert_eq!(SupportLang::from_extension("xyz"), None);
    }

    #[test]
    fn all_languages_have_file_types() {
        for lang in SupportLang::all() {
            assert!(!lang.file_types().is_empty(), "{lang:?} has no file types");
        }
    }
}
