//! Expansion queries - expand shrubbery to Content

use crate::durability::Durability;
use crate::queries::parse::ParseShrubberyQuery;
use crate::queries::source::DocId;
use crate::query::{Query, QueryDatabase};
use monowiki_mrl::{Content, Expander, TypeChecker};

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

        // Type check first
        let mut checker = TypeChecker::new();
        if let Err(e) = checker.check(&shrubbery) {
            return ExpandResult {
                content: None,
                errors: vec![format!("Type error: {}", e)],
            };
        }

        // Expand
        let mut expander = Expander::new();
        match expander.expand(&shrubbery) {
            Ok(content) => ExpandResult {
                content: Some(content),
                errors: vec![],
            },
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
    fn test_expand_simple_document() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId("test".to_string());

        storage.set_document(doc_id.clone(), "This is a test.".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let result = db.query::<ExpandToContentQuery>(doc_id);

        assert!(result.content.is_some(), "Expansion should succeed");
        assert!(result.errors.is_empty(), "Should have no errors");
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
}
