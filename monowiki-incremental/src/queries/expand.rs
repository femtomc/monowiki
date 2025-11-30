//! Expansion queries - process parsed content
//!
//! Provides both document-level and block-level expansion for fine-grained invalidation.

use crate::durability::Durability;
use crate::queries::parse::{ParseBlockQuery, ParseShrubberyQuery};
use crate::queries::source::{BlockId, DocId};
use crate::query::{Query, QueryDatabase};

/// Expanded content result
#[derive(Clone, Debug)]
pub struct ExpandResult {
    /// The expanded source text
    pub content: Option<String>,
    pub errors: Vec<String>,
}

impl std::hash::Hash for ExpandResult {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.content.is_some().hash(state);
        self.errors.len().hash(state);
    }
}

/// Query: Expand parsed content
pub struct ExpandToContentQuery;

impl Query for ExpandToContentQuery {
    type Key = DocId;
    type Value = ExpandResult;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Depend on parse query
        let parse_result = db.query::<ParseShrubberyQuery>(key.clone());

        match parse_result.source {
            Some(source) => ExpandResult {
                content: Some(source),
                errors: vec![],
            },
            None => ExpandResult {
                content: None,
                errors: parse_result.errors,
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

/// Query: Expand a single block's parsed content
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

        match parse_result.source {
            Some(source) => ExpandResult {
                content: Some(source),
                errors: vec![],
            },
            None => ExpandResult {
                content: None,
                errors: parse_result.errors,
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
    use crate::queries::source::SourceStorage;
    use std::sync::Arc;

    #[test]
    fn test_expand_query_runs() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId("test".to_string());

        storage.set_document(doc_id.clone(), "Hello world".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ExpandToContentQuery>(doc_id);

        assert!(result.content.is_some());
        assert!(result.errors.is_empty());
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

        storage.set_block(block_id.clone(), "Block content".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ExpandBlockQuery>(block_id);

        assert!(result.content.is_some());
        assert!(result.errors.is_empty());
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
}
