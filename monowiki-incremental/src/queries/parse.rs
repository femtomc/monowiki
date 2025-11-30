//! Parse queries - convert source text to parsed output
//!
//! Provides both document-level and block-level parsing for fine-grained invalidation.

use crate::durability::Durability;
use crate::queries::source::{BlockId, BlockSourceQuery, DocId, DocumentSourceQuery};
use crate::query::{Query, QueryDatabase};

/// Parsed result (or error)
#[derive(Clone, Debug)]
pub struct ParseResult {
    /// Raw source text that was parsed
    pub source: Option<String>,
    pub errors: Vec<String>,
}

impl std::hash::Hash for ParseResult {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.source.is_some().hash(state);
        self.errors.len().hash(state);
    }
}

/// Query: Parse document source
pub struct ParseShrubberyQuery;

impl Query for ParseShrubberyQuery {
    type Key = DocId;
    type Value = ParseResult;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Depend on source query
        let source = db.query::<DocumentSourceQuery>(key.clone());

        if source.is_empty() {
            return ParseResult {
                source: None,
                errors: vec!["Empty source".to_string()],
            };
        }

        ParseResult {
            source: Some(source),
            errors: vec![],
        }
    }

    fn durability() -> Durability {
        Durability::Volatile
    }

    fn name() -> &'static str {
        "ParseShrubberyQuery"
    }
}

/// Query: Parse a single block's source
///
/// This enables fine-grained invalidation - when a block changes,
/// only that block needs to be re-parsed.
pub struct ParseBlockQuery;

impl Query for ParseBlockQuery {
    type Key = BlockId;
    type Value = ParseResult;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Depend on block source query
        let source = db.query::<BlockSourceQuery>(key.clone());

        if source.is_empty() {
            return ParseResult {
                source: None,
                errors: vec!["Empty block source".to_string()],
            };
        }

        ParseResult {
            source: Some(source),
            errors: vec![],
        }
    }

    fn durability() -> Durability {
        Durability::Volatile
    }

    fn name() -> &'static str {
        "ParseBlockQuery"
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

        assert!(result.source.is_none());
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_parse_simple_document() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId("test".to_string());

        storage.set_document(doc_id.clone(), "This is prose.".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ParseShrubberyQuery>(doc_id);

        assert!(result.source.is_some(), "Parse should succeed");
        assert!(result.errors.is_empty(), "Should have no errors");
    }

    #[test]
    fn test_parse_block_simple() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(1);

        storage.set_block(block_id.clone(), "This is block content.".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ParseBlockQuery>(block_id);

        assert!(result.source.is_some(), "Block parse should succeed");
        assert!(result.errors.is_empty(), "Should have no errors");
    }

    #[test]
    fn test_parse_block_empty() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(1);

        // Empty block
        storage.set_block(block_id.clone(), "".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ParseBlockQuery>(block_id);

        assert!(result.source.is_none(), "Empty block should not parse");
        assert!(!result.errors.is_empty(), "Should have errors");
    }
}
