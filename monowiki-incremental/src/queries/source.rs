//! Source text queries - the entry point for document content

use crate::durability::Durability;
use crate::query::{Query, QueryDatabase};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

// Re-export shared types
pub use monowiki_types::{BlockId, DocId};

/// Storage for source text (input from CRDT or file)
pub struct SourceStorage {
    /// Document sources by ID
    documents: RwLock<HashMap<DocId, String>>,
    /// Block-level sources (for fine-grained invalidation)
    blocks: RwLock<HashMap<BlockId, String>>,
}

impl SourceStorage {
    pub fn new() -> Self {
        Self {
            documents: RwLock::new(HashMap::new()),
            blocks: RwLock::new(HashMap::new()),
        }
    }

    pub fn set_document(&self, doc_id: DocId, source: String) {
        self.documents.write().insert(doc_id, source);
    }

    pub fn get_document(&self, doc_id: &DocId) -> Option<String> {
        self.documents.read().get(doc_id).cloned()
    }

    pub fn set_block(&self, block_id: BlockId, source: String) {
        self.blocks.write().insert(block_id, source);
    }

    pub fn get_block(&self, block_id: &BlockId) -> Option<String> {
        self.blocks.read().get(block_id).cloned()
    }
}

impl Default for SourceStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Query: Get source text for a document
pub struct DocumentSourceQuery;

impl Query for DocumentSourceQuery {
    type Key = DocId;
    type Value = String;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        // Get from storage (would be set by CRDT layer)
        db.get_any("source_storage")
            .and_then(|any| any.downcast_ref::<Arc<SourceStorage>>())
            .and_then(|storage| storage.get_document(key))
            .unwrap_or_default()
    }

    fn durability() -> Durability {
        Durability::Volatile
    }

    fn name() -> &'static str {
        "DocumentSourceQuery"
    }
}

/// Query: Get source text for a specific block
pub struct BlockSourceQuery;

impl Query for BlockSourceQuery {
    type Key = BlockId;
    type Value = String;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        db.get_any("source_storage")
            .and_then(|any| any.downcast_ref::<Arc<SourceStorage>>())
            .and_then(|storage| storage.get_block(key))
            .unwrap_or_default()
    }

    fn durability() -> Durability {
        Durability::Volatile
    }

    fn name() -> &'static str {
        "BlockSourceQuery"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn test_source_storage() {
        let storage = SourceStorage::new();
        let doc_id = DocId::new("test");

        storage.set_document(doc_id.clone(), "# Hello".to_string());
        assert_eq!(storage.get_document(&doc_id), Some("# Hello".to_string()));
    }

    #[test]
    fn test_document_source_query() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId::new("test");

        storage.set_document(doc_id.clone(), "# Test".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let source = db.query::<DocumentSourceQuery>(doc_id);
        assert_eq!(source, "# Test");
    }
}
