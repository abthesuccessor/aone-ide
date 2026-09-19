use tree_sitter::Language;

use crate::domain::{CapabilityLevel, GraphEdge, GraphNode};

#[derive(Debug, Clone)]
pub struct LanguageSupport {
    pub name: &'static str,
    pub capability: CapabilityLevel,
    pub(super) parser: ParserKind,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ParserKind {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    Java,
    CSharp,
    Cpp,
    Kotlin,
    Document,
    Text,
}

impl ParserKind {
    pub(super) fn language(self) -> Option<Language> {
        match self {
            Self::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Self::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Self::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            Self::JavaScript => Some(tree_sitter_javascript::LANGUAGE.into()),
            Self::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Self::Go => Some(tree_sitter_go::LANGUAGE.into()),
            Self::Java => Some(tree_sitter_java::LANGUAGE.into()),
            Self::CSharp => Some(tree_sitter_c_sharp::LANGUAGE.into()),
            Self::Cpp => Some(tree_sitter_cpp::LANGUAGE.into()),
            Self::Kotlin => Some(tree_sitter_kotlin_ng::LANGUAGE.into()),
            Self::Document | Self::Text => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzedSource {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub parse_errors: bool,
    pub fact_count: usize,
}
