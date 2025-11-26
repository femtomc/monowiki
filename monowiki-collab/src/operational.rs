//! Operational document abstraction layer.
//!
//! This module provides a trait-based abstraction over different CRDT backends
//! (Yrs and Loro), enabling the editor to work with either implementation
//! while maintaining a consistent API.

use std::collections::HashMap;
use anyhow::Result;

/// Unique identifier for a node in the document tree.
pub type NodeId = String;

/// Unique identifier for a block in the document.
pub type BlockId = NodeId;

/// Unique identifier for a formatting mark.
pub type MarkId = String;

/// Unique identifier for a character in text.
pub type CharId = String;

/// Subscription handle for document change notifications.
pub type SubscriptionId = usize;

/// Fractional index for ordering siblings in the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct FractionalIndex(pub String);

impl FractionalIndex {
    pub fn new(index: String) -> Self {
        Self(index)
    }

    pub fn between(a: &Self, _b: &Self) -> Self {
        // Simple implementation - real fractional indexing is more complex
        Self(format!("{}.5", a.0))
    }

    pub fn first() -> Self {
        Self("0".to_string())
    }
}

/// The kind of block in the document.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockKind {
    Section,
    Heading { level: u8 },
    Paragraph,
    CodeBlock,
    List,
    ListItem,
    Blockquote,
}

/// A node in the document tree.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub kind: BlockKind,
    pub position: FractionalIndex,
    pub children: Vec<NodeId>,
}

/// The complete document tree structure.
#[derive(Debug, Clone)]
pub struct DocTree {
    pub root: NodeId,
    pub nodes: HashMap<NodeId, TreeNode>,
}

/// Anchor position for marks (Peritext-style).
#[derive(Debug, Clone, PartialEq)]
pub enum Anchor {
    Before,
    After,
}

/// A formatting mark on text.
#[derive(Debug, Clone)]
pub struct Mark {
    pub id: MarkId,
    pub mark_type: String,
    pub start: CharId,
    pub end: CharId,
    pub start_anchor: Anchor,
    pub end_anchor: Anchor,
    pub attrs: HashMap<String, String>,
}

/// Mark attributes (simplified).
pub type MarkAttrs = HashMap<String, String>;

/// A change to the document.
#[derive(Debug, Clone)]
pub enum DocChange {
    BlockInserted { block_id: BlockId },
    BlockMoved { block_id: BlockId },
    BlockDeleted { block_id: BlockId },
    TextInserted { block_id: BlockId, offset: u32, len: u32 },
    TextDeleted { block_id: BlockId, offset: u32, len: u32 },
    MarkAdded { block_id: BlockId, mark_id: MarkId },
    MarkRemoved { block_id: BlockId, mark_id: MarkId },
}

/// Abstract interface for operational document backends.
///
/// This trait provides a common API for both Yrs and Loro CRDT implementations,
/// allowing the editor to work with either backend transparently.
pub trait OperationalDoc: Send + Sync {
    /// Get the complete document tree structure.
    fn get_tree(&self) -> Result<DocTree>;

    /// Get the text content of a specific block.
    fn get_block_text(&self, block_id: BlockId) -> Result<String>;

    /// Get all formatting marks for a specific block.
    fn get_block_marks(&self, block_id: BlockId) -> Result<Vec<Mark>>;

    /// Insert a new block into the document tree.
    fn insert_block(
        &mut self,
        parent: NodeId,
        position: FractionalIndex,
        kind: BlockKind,
    ) -> Result<BlockId>;

    /// Move a block to a new position in the tree.
    fn move_block(
        &mut self,
        block_id: BlockId,
        new_parent: NodeId,
        new_position: FractionalIndex,
    ) -> Result<()>;

    /// Delete a block from the document (creates tombstone).
    fn delete_block(&mut self, block_id: BlockId) -> Result<()>;

    /// Insert text at a position within a block.
    fn insert_text(&mut self, block_id: BlockId, offset: u32, text: &str) -> Result<()>;

    /// Delete a range of text within a block.
    fn delete_text(&mut self, block_id: BlockId, start: u32, end: u32) -> Result<()>;

    /// Add a formatting mark to a text range.
    fn add_mark(
        &mut self,
        block_id: BlockId,
        mark_type: &str,
        start: u32,
        end: u32,
        attrs: MarkAttrs,
    ) -> Result<MarkId>;

    /// Remove a formatting mark.
    fn remove_mark(&mut self, block_id: BlockId, mark_id: MarkId) -> Result<()>;

    /// Encode the current document state for synchronization.
    fn encode_state(&self) -> Result<Vec<u8>>;

    /// Apply an update from a remote peer.
    fn apply_update(&mut self, update: &[u8]) -> Result<()>;

    /// Subscribe to document changes.
    ///
    /// Returns a subscription ID that can be used to unsubscribe.
    fn subscribe(&self, callback: Box<dyn Fn(DocChange) + Send>) -> SubscriptionId;

    /// Unsubscribe from document changes.
    fn unsubscribe(&self, subscription_id: SubscriptionId);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fractional_index_ordering() {
        let a = FractionalIndex::first();
        let b = FractionalIndex::new("1".to_string());
        let between = FractionalIndex::between(&a, &b);

        assert_ne!(a, between);
        assert_ne!(between, b);
    }

    #[test]
    fn test_block_kind_variants() {
        let heading = BlockKind::Heading { level: 1 };
        let para = BlockKind::Paragraph;

        assert_ne!(
            std::mem::discriminant(&heading),
            std::mem::discriminant(&para)
        );
    }
}
