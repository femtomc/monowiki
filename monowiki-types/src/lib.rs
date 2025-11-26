//! Shared types for monowiki
//!
//! This crate provides common types used across the monowiki ecosystem,
//! including document identifiers and change events.

use serde::{Deserialize, Serialize};

/// Document identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DocId(pub String);

impl DocId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Block identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlockId(pub u64);

impl BlockId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

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

/// Document change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocChange {
    /// Text content changed in a block
    TextChanged {
        block_id: BlockId,
        start: usize,
        end: usize,
        new_text: String,
    },

    /// A block was moved in the tree
    BlockMoved {
        block_id: BlockId,
        old_parent: BlockId,
        new_parent: BlockId,
        position: usize,
    },

    /// A new block was inserted
    BlockInserted {
        block_id: BlockId,
        parent_id: BlockId,
        position: usize,
    },

    /// A block was deleted
    BlockDeleted { block_id: BlockId },

    /// Formatting marks changed
    MarkChanged {
        block_id: BlockId,
        mark_type: String,
        start: usize,
        end: usize,
    },
}

/// Source span for error reporting and source tracking
///
/// Represents a range in source text, used for error messages
/// and tracking where content came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Merge two spans to create a span that covers both
    pub fn merge(&self, other: &Span) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Get the length of this span
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Check if this span is empty
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Content kind for type system
///
/// Used in the MRL type system to distinguish between
/// block-level and inline content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentKind {
    Block,
    Inline,
    Content,
}

impl ContentKind {
    /// Check if this kind is a subkind of another
    pub fn is_subkind_of(&self, other: &ContentKind) -> bool {
        match (self, other) {
            (ContentKind::Block, ContentKind::Content) => true,
            (ContentKind::Inline, ContentKind::Content) => true,
            (a, b) => a == b,
        }
    }
}

impl std::fmt::Display for ContentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentKind::Block => write!(f, "Block"),
            ContentKind::Inline => write!(f, "Inline"),
            ContentKind::Content => write!(f, "Content"),
        }
    }
}
