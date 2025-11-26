//! CRDT invalidation bridge
//!
//! This module bridges CRDT changes to query invalidation, ensuring
//! that the incremental computation system stays synchronized with
//! collaborative edits.

use crate::db::Db;
use crate::query::Query;
use std::sync::Arc;

/// Represents a change to a document in the CRDT layer
#[derive(Debug, Clone)]
pub enum DocChange {
    /// Text content changed in a block
    TextChanged {
        block_id: BlockId,
        range: TextRange,
        new_text: String,
    },

    /// A block was moved in the tree
    BlockMoved {
        block_id: BlockId,
        old_parent: BlockId,
        new_parent: BlockId,
        new_index: usize,
    },

    /// A new block was inserted
    BlockInserted {
        block_id: BlockId,
        parent_id: BlockId,
        index: usize,
        block_type: BlockType,
    },

    /// A block was deleted
    BlockDeleted { block_id: BlockId },

    /// Formatting marks changed
    MarkChanged {
        block_id: BlockId,
        mark_type: String,
        range: TextRange,
    },

    /// Document metadata changed
    MetadataChanged { key: String, value: String },
}

/// Block identifier in the document tree
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u64);

impl From<u64> for BlockId {
    fn from(id: u64) -> Self {
        BlockId(id)
    }
}

impl From<BlockId> for u64 {
    fn from(id: BlockId) -> Self {
        id.0
    }
}

/// Section identifier (for compatibility with existing types)
pub type SectionId = BlockId;

/// Type of a block in the document
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockType {
    Heading,
    Paragraph,
    CodeBlock,
    List,
    ListItem,
    Blockquote,
    ThematicBreak,
}

/// Range in text content
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub fn new(start: usize, end: usize) -> Self {
        TextRange { start, end }
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Bridge between CRDT changes and incremental queries
pub struct InvalidationBridge {
    /// Reference to the query database
    db: Arc<Db>,
}

impl InvalidationBridge {
    /// Create a new invalidation bridge
    pub fn new(db: Arc<Db>) -> Self {
        InvalidationBridge { db }
    }

    /// Handle a CRDT change and invalidate affected queries
    pub fn on_crdt_change(&self, change: DocChange) {
        match change {
            DocChange::TextChanged { block_id, .. } => {
                // Invalidate source text query for this block
                self.invalidate_source_text(block_id);
            }

            DocChange::BlockMoved { .. }
            | DocChange::BlockInserted { .. }
            | DocChange::BlockDeleted { .. } => {
                // Invalidate tree structure queries
                self.invalidate_doc_tree();
            }

            DocChange::MarkChanged { block_id, .. } => {
                // Marks affect parsing, so invalidate source
                self.invalidate_source_text(block_id);
            }

            DocChange::MetadataChanged { .. } => {
                // Invalidate metadata queries
                self.invalidate_doc_metadata();
            }
        }
    }

    /// Invalidate source text for a specific block
    fn invalidate_source_text(&self, _block_id: BlockId) {
        // In a real implementation, we would invalidate the specific
        // SourceTextQuery for this block. For now, this is a placeholder.
        //
        // self.db.invalidate::<SourceTextQuery>(block_id.into());
    }

    /// Invalidate the document tree structure
    fn invalidate_doc_tree(&self) {
        // In a real implementation, we would invalidate DocTreeQuery
        //
        // self.db.invalidate::<DocTreeQuery>(());
    }

    /// Invalidate document metadata
    fn invalidate_doc_metadata(&self) {
        // In a real implementation, we would invalidate MetadataQuery
        //
        // self.db.invalidate::<MetadataQuery>(());
    }

    /// Process a batch of changes efficiently
    pub fn on_crdt_changes(&self, changes: Vec<DocChange>) {
        // Collect unique invalidations to avoid redundant work
        let mut invalidated_blocks = std::collections::HashSet::new();
        let mut tree_changed = false;
        let mut metadata_changed = false;

        for change in changes {
            match change {
                DocChange::TextChanged { block_id, .. }
                | DocChange::MarkChanged { block_id, .. } => {
                    invalidated_blocks.insert(block_id);
                }

                DocChange::BlockMoved { .. }
                | DocChange::BlockInserted { .. }
                | DocChange::BlockDeleted { .. } => {
                    tree_changed = true;
                }

                DocChange::MetadataChanged { .. } => {
                    metadata_changed = true;
                }
            }
        }

        // Apply invalidations
        for block_id in invalidated_blocks {
            self.invalidate_source_text(block_id);
        }

        if tree_changed {
            self.invalidate_doc_tree();
        }

        if metadata_changed {
            self.invalidate_doc_metadata();
        }
    }
}

/// Helper trait for converting CRDT events to DocChange
pub trait CrdtEvent {
    fn to_doc_change(&self) -> Option<DocChange>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_id() {
        let id1 = BlockId(42);
        let id2: BlockId = 42.into();

        assert_eq!(id1, id2);
        assert_eq!(u64::from(id1), 42);
    }

    #[test]
    fn test_text_range() {
        let range = TextRange::new(5, 10);

        assert_eq!(range.len(), 5);
        assert!(!range.is_empty());

        let empty = TextRange::new(5, 5);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_invalidation_bridge_creation() {
        let db = Arc::new(Db::new());
        let bridge = InvalidationBridge::new(db);

        // Just test that we can create it
        drop(bridge);
    }

    #[test]
    fn test_batch_changes() {
        let db = Arc::new(Db::new());
        let bridge = InvalidationBridge::new(db);

        let changes = vec![
            DocChange::TextChanged {
                block_id: BlockId(1),
                range: TextRange::new(0, 5),
                new_text: "hello".to_string(),
            },
            DocChange::TextChanged {
                block_id: BlockId(1),
                range: TextRange::new(5, 10),
                new_text: " world".to_string(),
            },
            DocChange::BlockInserted {
                block_id: BlockId(2),
                parent_id: BlockId(0),
                index: 1,
                block_type: BlockType::Paragraph,
            },
        ];

        // Should not panic
        bridge.on_crdt_changes(changes);
    }
}
