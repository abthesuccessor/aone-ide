use std::path::Path;

use crate::domain::CapabilityLevel;

use super::{LanguageSupport, types::ParserKind};

pub fn language_support(path: &Path) -> LanguageSupport {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "rs" => syntax("Rust", ParserKind::Rust),
        "ts" | "mts" | "cts" => semantic("TypeScript", ParserKind::TypeScript),
        "tsx" => semantic("TypeScript", ParserKind::Tsx),
        "js" | "mjs" | "cjs" | "jsx" => semantic("JavaScript", ParserKind::JavaScript),
        "py" | "pyi" => syntax("Python", ParserKind::Python),
        "go" => syntax("Go", ParserKind::Go),
        "java" => syntax("Java", ParserKind::Java),
        "cs" => syntax("C#", ParserKind::CSharp),
        "c" | "cc" | "cpp" | "cxx" | "h" | "hh" | "hpp" | "hxx" => syntax("C++", ParserKind::Cpp),
        "kt" | "kts" => syntax("Kotlin", ParserKind::Kotlin),
        "json" => text("JSON"),
        "toml" => text("TOML"),
        "yaml" | "yml" => text("YAML"),
        "md" | "mdx" => document("Markdown"),
        "txt" => document("Text"),
        "rst" => document("reStructuredText"),
        "adoc" | "asciidoc" => document("AsciiDoc"),
        "sql" => text("SQL"),
        "graphql" | "gql" => text("GraphQL"),
        "proto" => text("Protocol Buffers"),
        "sh" | "bash" | "zsh" => text("Shell"),
        "html" | "htm" => text("HTML"),
        "css" | "scss" | "sass" | "less" => text("CSS"),
        _ => text("Text"),
    }
}

fn syntax(name: &'static str, parser: ParserKind) -> LanguageSupport {
    LanguageSupport {
        name,
        capability: CapabilityLevel::SyntaxOnly,
        parser,
    }
}

fn semantic(name: &'static str, parser: ParserKind) -> LanguageSupport {
    LanguageSupport {
        name,
        capability: CapabilityLevel::Semantic,
        parser,
    }
}

fn text(name: &'static str) -> LanguageSupport {
    LanguageSupport {
        name,
        capability: CapabilityLevel::TextOnly,
        parser: ParserKind::Text,
    }
}

fn document(name: &'static str) -> LanguageSupport {
    LanguageSupport {
        name,
        capability: CapabilityLevel::TextOnly,
        parser: ParserKind::Document,
    }
}
