//! Bridge between CRDT changes and query invalidation

use crate::db::Db;
use crate::queries::expand::ExpandToContentQuery;
use crate::queries::parse::ParseShrubberyQuery;
use crate::queries::source::{BlockId, BlockSourceQuery, DocId, DocumentSourceQuery};
use monowiki_types::DocChange;
use std::sync::Arc;

/// Bridge that converts CRDT changes to query invalidations
pub struct InvalidationBridge {
    db: Arc<Db>,
}

impl InvalidationBridge {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Handle a document change event from the CRDT layer
    pub fn on_change(&self, doc_id: &DocId, change: DocChange) {
        match change {
            DocChange::TextChanged { block_id, .. } => {
                // Invalidate block source
                self.db.invalidate::<BlockSourceQuery>(block_id);
                // Also invalidate document-level queries
                self.db
                    .invalidate::<DocumentSourceQuery>(doc_id.clone());
                self.db.invalidate::<ParseShrubberyQuery>(doc_id.clone());
                self.db
                    .invalidate::<ExpandToContentQuery>(doc_id.clone());
            }
            DocChange::BlockInserted { .. }
            | DocChange::BlockDeleted { .. }
            | DocChange::BlockMoved { .. } => {
                // Structure change - invalidate everything for this doc
                self.db
                    .invalidate::<DocumentSourceQuery>(doc_id.clone());
                self.db.invalidate::<ParseShrubberyQuery>(doc_id.clone());
                self.db
                    .invalidate::<ExpandToContentQuery>(doc_id.clone());
            }
            DocChange::MarkChanged { block_id, .. } => {
                // Mark changes affect parsing
                self.db.invalidate::<BlockSourceQuery>(block_id);
                self.db
                    .invalidate::<DocumentSourceQuery>(doc_id.clone());
                self.db.invalidate::<ParseShrubberyQuery>(doc_id.clone());
            }
        }
    }

    /// Batch invalidation for multiple changes
    pub fn on_changes(&self, doc_id: &DocId, changes: Vec<DocChange>) {
        for change in changes {
            self.on_change(doc_id, change);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalidation_bridge_creation() {
        let db = Arc::new(Db::new());
        let bridge = InvalidationBridge::new(db);
        drop(bridge);
    }

    #[test]
    fn test_text_change_invalidation() {
        let db = Arc::new(Db::new());
        let bridge = InvalidationBridge::new(db.clone());

        let doc_id = DocId::new("test");
        let block_id = BlockId::new(1);

        let rev1 = db.revision();

        bridge.on_change(
            &doc_id,
            DocChange::TextChanged {
                block_id,
                start: 0,
                end: 5,
                new_text: "hello".to_string(),
            },
        );

        let rev2 = db.revision();
        assert!(rev2.0 > rev1.0, "Revision should increase after invalidation");
    }

    #[test]
    fn test_batch_changes() {
        let db = Arc::new(Db::new());
        let bridge = InvalidationBridge::new(db);

        let doc_id = DocId::new("test");
        let changes = vec![
            DocChange::TextChanged {
                block_id: BlockId::new(1),
                start: 0,
                end: 5,
                new_text: "hello".to_string(),
            },
            DocChange::BlockInserted {
                block_id: BlockId::new(2),
                parent_id: BlockId::new(0),
                position: 1,
            },
        ];

        bridge.on_changes(&doc_id, changes);
    }
}
