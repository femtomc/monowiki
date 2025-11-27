//! Parse queries - convert source text to shrubbery
//!
//! Provides both document-level and block-level parsing for fine-grained invalidation.

use crate::durability::Durability;
use crate::queries::source::{BlockId, BlockSourceQuery, DocId, DocumentSourceQuery};
use crate::query::{Query, QueryDatabase};
use monowiki_mrl::{parse_with_symbols, tokenize, Shrubbery, SymbolTable};

/// Parsed shrubbery result (or error)
#[derive(Clone, Debug)]
pub struct ParseResult {
    pub shrubbery: Option<Shrubbery>,
    /// Symbol table from parsing (maps names to symbols)
    pub symbols: Option<SymbolTable>,
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
                symbols: None,
                errors: vec!["Empty source".to_string()],
            };
        }

        // Tokenize
        let tokens = match tokenize(&source) {
            Ok(t) => t,
            Err(e) => {
                return ParseResult {
                    shrubbery: None,
                    symbols: None,
                    errors: vec![format!("Lexer error: {}", e)],
                };
            }
        };

        // Parse with symbols
        match parse_with_symbols(&tokens) {
            Ok((shrub, symbols)) => ParseResult {
                shrubbery: Some(shrub),
                symbols: Some(symbols),
                errors: vec![],
            },
            Err(e) => ParseResult {
                shrubbery: None,
                symbols: None,
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

/// Query: Parse a single block's source into shrubbery
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
                shrubbery: None,
                symbols: None,
                errors: vec!["Empty block source".to_string()],
            };
        }

        // Tokenize
        let tokens = match tokenize(&source) {
            Ok(t) => t,
            Err(e) => {
                return ParseResult {
                    shrubbery: None,
                    symbols: None,
                    errors: vec![format!("Lexer error in block {:?}: {}", key, e)],
                };
            }
        };

        // Parse with symbols
        match parse_with_symbols(&tokens) {
            Ok((shrub, symbols)) => ParseResult {
                shrubbery: Some(shrub),
                symbols: Some(symbols),
                errors: vec![],
            },
            Err(e) => ParseResult {
                shrubbery: None,
                symbols: None,
                errors: vec![format!("Parser error in block {:?}: {}", key, e)],
            },
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

    #[test]
    fn test_parse_block_simple() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(1);

        storage.set_block(block_id.clone(), "This is block content.".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ParseBlockQuery>(block_id);

        assert!(result.shrubbery.is_some(), "Block parse should succeed");
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

        assert!(result.shrubbery.is_none(), "Empty block should not parse");
        assert!(!result.errors.is_empty(), "Should have errors");
    }

    #[test]
    fn test_parse_block_with_macro() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(42);

        storage.set_block(
            block_id.clone(),
            "Text with !emphasis([styled])!.".to_string(),
        );
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ParseBlockQuery>(block_id);

        assert!(result.shrubbery.is_some(), "Block with macro should parse");
    }
}
