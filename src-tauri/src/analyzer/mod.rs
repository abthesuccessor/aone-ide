mod api;
mod api_definition;
mod api_handler;
mod api_javascript;
mod deadline;
mod document;
mod document_structure;
mod extract;
mod extract_support;
mod ids;
mod language;
mod openapi;
mod syntax;
mod text;
mod types;

#[cfg(test)]
mod api_tests;
#[cfg(test)]
mod document_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub(crate) use extract::analyze_source_with_deadline;
pub use ids::stable_id;
pub use language::language_support;
pub use types::{AnalyzedSource, LanguageSupport};

#[cfg(test)]
pub use test_support::analyze_source;
