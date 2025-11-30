//! Document processing pipeline connecting all crates
//!
//! Flow: Source → Parse → Render
//!       ↑                    ↓
//!       CRDT ←─── Query ───→ Cache

use monowiki_types::{DocChange, DocId};
use std::sync::Arc;

/// The main document processing pipeline
///
/// This connects all the monowiki crates into a unified end-to-end
/// document transformation system.
pub struct DocumentPipeline {
    /// Incremental query database
    db: Arc<monowiki_incremental::Db>,
}

impl DocumentPipeline {
    /// Create a new document pipeline
    pub fn new() -> Self {
        Self {
            db: Arc::new(monowiki_incremental::Db::new()),
        }
    }

    /// Create a pipeline with access to the incremental database
    pub fn with_db(db: Arc<monowiki_incremental::Db>) -> Self {
        Self { db }
    }

    /// Get a reference to the incremental database
    pub fn db(&self) -> &Arc<monowiki_incremental::Db> {
        &self.db
    }

    /// Handle a CRDT change event
    ///
    /// This invalidates affected queries in the incremental system,
    /// ensuring that subsequent queries recompute as needed.
    pub fn on_change(&self, doc_id: &DocId, change: DocChange) {
        use monowiki_incremental::InvalidationBridge;

        let bridge = InvalidationBridge::new(self.db.clone());
        bridge.on_change(doc_id, change);
    }

    /// Handle multiple CRDT changes efficiently
    pub fn on_changes(&self, doc_id: &DocId, changes: Vec<DocChange>) {
        use monowiki_incremental::InvalidationBridge;

        let bridge = InvalidationBridge::new(self.db.clone());
        bridge.on_changes(doc_id, changes);
    }
}

impl Default for DocumentPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur in the document pipeline
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// Error from CRDT layer
    #[error("CRDT error: {0}")]
    Crdt(#[from] anyhow::Error),

    /// Generic pipeline error
    #[error("Pipeline error: {0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use monowiki_types::BlockId;

    #[test]
    fn test_pipeline_creation() {
        let pipeline = DocumentPipeline::new();
        assert!(Arc::strong_count(pipeline.db()) >= 1);
    }

    #[test]
    fn test_invalidation() {
        let doc_id = DocId::new("test-doc");
        let pipeline = DocumentPipeline::new();

        // Make a change
        let change = DocChange::TextChanged {
            block_id: BlockId::new(1),
            start: 0,
            end: 5,
            new_text: "hello".to_string(),
        };

        // Should not panic
        pipeline.on_change(&doc_id, change);
    }
}
