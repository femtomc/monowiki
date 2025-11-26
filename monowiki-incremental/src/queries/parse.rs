//! Parse queries - convert source text to shrubbery

use crate::durability::Durability;
use crate::queries::source::{DocId, DocumentSourceQuery};
use crate::query::{Query, QueryDatabase};
use monowiki_mrl::{parse, tokenize, Shrubbery};

/// Parsed shrubbery result (or error)
#[derive(Clone, Debug)]
pub struct ParseResult {
    pub shrubbery: Option<Shrubbery>,
    pub errors: Vec<String>,
}

impl std::hash::Hash for ParseResult {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash based on success/failure and error count
        self.shrubbery.is_some().hash(state);
        self.errors.len().hash(state);
    }
}

/// Query: Parse document source into shrubbery
pub struct ParseShrubberyQuery;

impl Query for ParseShrubberyQuery {
    type Key = DocId;
    type Value = ParseResult;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Depend on source query
        let source = db.query::<DocumentSourceQuery>(key.clone());

        if source.is_empty() {
            return ParseResult {
                shrubbery: None,
                errors: vec!["Empty source".to_string()],
            };
        }

        // Tokenize
        let tokens = match tokenize(&source) {
            Ok(t) => t,
            Err(e) => {
                return ParseResult {
                    shrubbery: None,
                    errors: vec![format!("Lexer error: {}", e)],
                };
            }
        };

        // Parse
        match parse(&tokens) {
            Ok(shrub) => ParseResult {
                shrubbery: Some(shrub),
                errors: vec![],
            },
            Err(e) => ParseResult {
                shrubbery: None,
                errors: vec![format!("Parser error: {}", e)],
            },
        }
    }

    fn durability() -> Durability {
        Durability::Volatile
    }

    fn name() -> &'static str {
        "ParseShrubberyQuery"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::queries::source::SourceStorage;
    use std::sync::Arc;

    #[test]
    fn test_parse_empty_source() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let doc_id = DocId("test".to_string());
        let result = db.query::<ParseShrubberyQuery>(doc_id);

        assert!(result.shrubbery.is_none());
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_parse_simple_document() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId("test".to_string());

        // Simple MRL document
        storage.set_document(doc_id.clone(), "This is prose.".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ParseShrubberyQuery>(doc_id);

        assert!(result.shrubbery.is_some(), "Parse should succeed");
        assert!(result.errors.is_empty(), "Should have no errors");
    }

    #[test]
    fn test_parse_with_code() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId("test".to_string());

        // MRL with inline code
        storage.set_document(
            doc_id.clone(),
            "Hello !bold([world])!".to_string(),
        );
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ParseShrubberyQuery>(doc_id);

        assert!(result.shrubbery.is_some(), "Parse should succeed");
    }
}
