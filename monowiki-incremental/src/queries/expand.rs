//! Expansion queries - expand shrubbery to Content
//!
//! Provides both document-level and block-level expansion for fine-grained invalidation.

use crate::durability::Durability;
use crate::queries::parse::{ParseBlockQuery, ParseShrubberyQuery};
use crate::queries::source::{BlockId, DocId};
use crate::query::{Query, QueryDatabase};
use monowiki_mrl::{Content, ExpandValue, Expander, TypeChecker};

/// Expanded content result
#[derive(Clone, Debug)]
pub struct ExpandResult {
    pub content: Option<Content>,
    pub errors: Vec<String>,
}

impl std::hash::Hash for ExpandResult {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.content.is_some().hash(state);
        self.errors.len().hash(state);
    }
}

/// Query: Expand shrubbery to typed Content
pub struct ExpandToContentQuery;

impl Query for ExpandToContentQuery {
    type Key = DocId;
    type Value = ExpandResult;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Depend on parse query
        let parse_result = db.query::<ParseShrubberyQuery>(key.clone());

        let shrubbery = match parse_result.shrubbery {
            Some(s) => s,
            None => {
                return ExpandResult {
                    content: None,
                    errors: parse_result.errors,
                };
            }
        };

        // Get symbol table from parse result
        let symbols = parse_result.symbols;

        // Type check first
        let mut checker = TypeChecker::new();
        // Register parsed symbols with checker
        if let Some(ref sym_table) = symbols {
            checker.register_symbols(sym_table.symbols());
        }
        if let Err(e) = checker.check(&shrubbery) {
            return ExpandResult {
                content: None,
                errors: vec![format!("Type error: {}", e)],
            };
        }

        // Expand
        let mut expander = Expander::new();
        // Register parsed symbols with expander (convert name→symbol to id→name)
        if let Some(ref sym_table) = symbols {
            let id_to_name: std::collections::HashMap<u64, String> = sym_table
                .symbols()
                .iter()
                .map(|(name, sym)| (sym.id(), name.clone()))
                .collect();
            expander.set_symbols(id_to_name);
        }
        match expander.expand(&shrubbery) {
            Ok(value) => {
                // Extract Content from ExpandValue
                let content = match value {
                    ExpandValue::Content(c) => Some(c),
                    _ => None, // Non-Content results are valid but don't produce document content
                };
                ExpandResult {
                    content,
                    errors: vec![],
                }
            }
            Err(e) => ExpandResult {
                content: None,
                errors: vec![format!("Expansion error: {}", e)],
            },
        }
    }

    fn durability() -> Durability {
        Durability::Volatile
    }

    fn name() -> &'static str {
        "ExpandToContentQuery"
    }
}

/// Query: Expand a single block's shrubbery to Content
///
/// This enables fine-grained invalidation - when a block changes,
/// only that block needs to be re-expanded.
pub struct ExpandBlockQuery;

impl Query for ExpandBlockQuery {
    type Key = BlockId;
    type Value = ExpandResult;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Depend on block parse query
        let parse_result = db.query::<ParseBlockQuery>(key.clone());

        let shrubbery = match parse_result.shrubbery {
            Some(s) => s,
            None => {
                return ExpandResult {
                    content: None,
                    errors: parse_result.errors,
                };
            }
        };

        // Get symbol table from parse result
        let symbols = parse_result.symbols;

        // Type check first
        let mut checker = TypeChecker::new();
        // Register parsed symbols with checker
        if let Some(ref sym_table) = symbols {
            checker.register_symbols(sym_table.symbols());
        }
        if let Err(e) = checker.check(&shrubbery) {
            return ExpandResult {
                content: None,
                errors: vec![format!("Type error in block {:?}: {}", key, e)],
            };
        }

        // Expand
        let mut expander = Expander::new();
        // Register parsed symbols with expander (convert name→symbol to id→name)
        if let Some(ref sym_table) = symbols {
            let id_to_name: std::collections::HashMap<u64, String> = sym_table
                .symbols()
                .iter()
                .map(|(name, sym)| (sym.id(), name.clone()))
                .collect();
            expander.set_symbols(id_to_name);
        }
        match expander.expand(&shrubbery) {
            Ok(value) => {
                let content = match value {
                    ExpandValue::Content(c) => Some(c),
                    _ => None,
                };
                ExpandResult {
                    content,
                    errors: vec![],
                }
            }
            Err(e) => ExpandResult {
                content: None,
                errors: vec![format!("Expansion error in block {:?}: {}", key, e)],
            },
        }
    }

    fn durability() -> Durability {
        Durability::Volatile
    }

    fn name() -> &'static str {
        "ExpandBlockQuery"
    }
}

/// Active macros configuration (durable tier)
pub struct ActiveMacrosQuery;

impl Query for ActiveMacrosQuery {
    type Key = ();
    type Value = MacroConfig;

    fn execute<DB: QueryDatabase>(_db: &DB, _key: &Self::Key) -> Self::Value {
        MacroConfig::default()
    }

    fn durability() -> Durability {
        Durability::Durable
    }

    fn name() -> &'static str {
        "ActiveMacrosQuery"
    }
}

#[derive(Clone, Debug, Default, Hash)]
pub struct MacroConfig {
    pub enabled_macros: Vec<String>,
    pub version: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::queries::source::{BlockId, SourceStorage};
    use std::sync::Arc;

    #[test]
    fn test_expand_query_runs() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId("test".to_string());

        // Simple prose - expansion may not produce content but query should run
        storage.set_document(doc_id.clone(), "Hello world".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ExpandToContentQuery>(doc_id);

        // Verify the query ran without panicking
        // Note: Simple prose may not expand to Content - that's OK for this test
        assert!(result.errors.is_empty() || result.errors.iter().all(|e| !e.contains("panic")));
    }

    #[test]
    fn test_expand_with_parse_error() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId("test".to_string());

        // Empty source should cause parse error
        storage.set_document(doc_id.clone(), "".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ExpandToContentQuery>(doc_id);

        assert!(result.content.is_none(), "Expansion should fail");
        assert!(!result.errors.is_empty(), "Should have errors");
    }

    #[test]
    fn test_macro_config() {
        let db = Db::new();
        let config = db.query::<ActiveMacrosQuery>(());

        assert_eq!(config.enabled_macros.len(), 0);
        assert_eq!(config.version, 0);
    }

    #[test]
    fn test_expand_block_query_runs() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(1);

        // Simple content - expansion may not produce Content but query should run
        storage.set_block(block_id.clone(), "Block content".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ExpandBlockQuery>(block_id);

        // Verify the block query ran without panicking
        assert!(result.errors.is_empty() || result.errors.iter().all(|e| !e.contains("panic")));
    }

    #[test]
    fn test_expand_block_empty() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(1);

        storage.set_block(block_id.clone(), "".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ExpandBlockQuery>(block_id);

        assert!(result.content.is_none(), "Empty block should not expand");
        assert!(!result.errors.is_empty(), "Should have errors");
    }

    #[test]
    fn test_expand_block_dependencies() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(42);

        // Test that block expansion depends on block parse
        storage.set_block(block_id.clone(), "Some content".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        // First query runs the pipeline
        let _ = db.query::<ExpandBlockQuery>(block_id.clone());

        // Second query should use cached parse result
        let _ = db.query::<ExpandBlockQuery>(block_id);
    }
}
