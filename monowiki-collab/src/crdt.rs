//! Loro-based CRDT document store for collaborative editing.
//!
//! Three-layer CRDT architecture:
//! - **MovableTree**: Hierarchical document structure (sections/blocks)
//! - **Richtext (Fugue)**: Per-block text sequences with causal ordering
//! - **Peritext marks**: Rich text formatting with anchor semantics
//!
//! Based on the design in vault/design/design.md Section 7.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use loro::{ExportMode, LoroDoc, LoroList, LoroMap, LoroText, LoroValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use parking_lot::RwLock as SyncRwLock;
use tokio::sync::{broadcast, RwLock};

// =============================================================================
// Block Types
// =============================================================================

/// Types of blocks in the document structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    /// Section container (groups blocks under a heading)
    Section,
    /// Heading (level 1-6)
    Heading,
    /// Paragraph
    Paragraph,
    /// Code block (with optional language)
    CodeBlock,
    /// Bullet list
    BulletList,
    /// Ordered list
    OrderedList,
    /// List item
    ListItem,
    /// Block quote
    Blockquote,
    /// Thematic break (horizontal rule)
    ThematicBreak,
    /// Math block (display mode)
    MathBlock,
    /// Table
    Table,
    /// Table row
    TableRow,
    /// Table cell
    TableCell,
}

impl BlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockKind::Section => "section",
            BlockKind::Heading => "heading",
            BlockKind::Paragraph => "paragraph",
            BlockKind::CodeBlock => "code_block",
            BlockKind::BulletList => "bullet_list",
            BlockKind::OrderedList => "ordered_list",
            BlockKind::ListItem => "list_item",
            BlockKind::Blockquote => "blockquote",
            BlockKind::ThematicBreak => "thematic_break",
            BlockKind::MathBlock => "math_block",
            BlockKind::Table => "table",
            BlockKind::TableRow => "table_row",
            BlockKind::TableCell => "table_cell",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "section" => Some(BlockKind::Section),
            "heading" => Some(BlockKind::Heading),
            "paragraph" => Some(BlockKind::Paragraph),
            "code_block" => Some(BlockKind::CodeBlock),
            "bullet_list" => Some(BlockKind::BulletList),
            "ordered_list" => Some(BlockKind::OrderedList),
            "list_item" => Some(BlockKind::ListItem),
            "blockquote" => Some(BlockKind::Blockquote),
            "thematic_break" => Some(BlockKind::ThematicBreak),
            "math_block" => Some(BlockKind::MathBlock),
            "table" => Some(BlockKind::Table),
            "table_row" => Some(BlockKind::TableRow),
            "table_cell" => Some(BlockKind::TableCell),
            _ => None,
        }
    }

    /// Whether this block kind contains text
    pub fn has_text(&self) -> bool {
        matches!(
            self,
            BlockKind::Heading
                | BlockKind::Paragraph
                | BlockKind::CodeBlock
                | BlockKind::ListItem
                | BlockKind::MathBlock
                | BlockKind::TableCell
        )
    }

    /// Whether this block kind can have children
    pub fn can_have_children(&self) -> bool {
        matches!(
            self,
            BlockKind::Section
                | BlockKind::BulletList
                | BlockKind::OrderedList
                | BlockKind::Blockquote
                | BlockKind::Table
                | BlockKind::TableRow
        )
    }
}

/// Anchor behavior for Peritext marks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    /// Insert before this boundary expands the mark
    Before,
    /// Insert after this boundary expands the mark
    After,
}

/// A formatting mark with Peritext-style anchor semantics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mark {
    /// Mark type (e.g., "emphasis", "strong", "link", "code")
    pub mark_type: String,
    /// Start position (character index)
    pub start: usize,
    /// End position (character index)
    pub end: usize,
    /// Start anchor behavior
    pub start_anchor: Anchor,
    /// End anchor behavior
    pub end_anchor: Anchor,
    /// Additional attributes (e.g., href for links)
    #[serde(default)]
    pub attrs: HashMap<String, Value>,
}

/// A comment/annotation anchored to a text range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    /// Unique comment ID
    pub id: String,
    /// Block this comment is attached to
    pub block_id: String,
    /// Start position within the block
    pub start: usize,
    /// End position within the block
    pub end: usize,
    /// Comment content
    pub content: String,
    /// Author (user ID or "agent")
    pub author: String,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Whether this comment is resolved
    pub resolved: bool,
    /// Optional thread parent (for replies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

impl Comment {
    pub fn new(
        id: impl Into<String>,
        block_id: impl Into<String>,
        start: usize,
        end: usize,
        content: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            block_id: block_id.into(),
            start,
            end,
            content: content.into(),
            author: author.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            resolved: false,
            parent_id: None,
        }
    }

    pub fn reply(
        id: impl Into<String>,
        parent: &Comment,
        content: impl Into<String>,
        author: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            block_id: parent.block_id.clone(),
            start: parent.start,
            end: parent.end,
            content: content.into(),
            author: author.into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            resolved: false,
            parent_id: Some(parent.id.clone()),
        }
    }
}

impl Mark {
    pub fn new(mark_type: impl Into<String>, start: usize, end: usize) -> Self {
        Self {
            mark_type: mark_type.into(),
            start,
            end,
            // Default Peritext behavior: expand on both ends
            start_anchor: Anchor::Before,
            end_anchor: Anchor::After,
            attrs: HashMap::new(),
        }
    }

    pub fn with_anchor(mut self, start: Anchor, end: Anchor) -> Self {
        self.start_anchor = start;
        self.end_anchor = end;
        self
    }

    pub fn with_attr(mut self, key: impl Into<String>, value: Value) -> Self {
        self.attrs.insert(key.into(), value);
        self
    }
}

/// Block metadata stored in the tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMeta {
    pub kind: BlockKind,
    /// Block-specific attributes (e.g., level for headings, language for code)
    #[serde(default)]
    pub attrs: HashMap<String, Value>,
}

impl BlockMeta {
    pub fn new(kind: BlockKind) -> Self {
        Self {
            kind,
            attrs: HashMap::new(),
        }
    }

    pub fn with_attr(mut self, key: impl Into<String>, value: Value) -> Self {
        self.attrs.insert(key.into(), value);
        self
    }
}

/// Block representation for storage and retrieval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockData {
    /// Unique block ID
    pub id: String,
    /// Block kind (heading, paragraph, code_block, etc.)
    pub kind: String,
    /// Block-specific attributes
    #[serde(default)]
    pub attrs: HashMap<String, Value>,
    /// Text content of the block
    #[serde(default)]
    pub text: String,
}

// =============================================================================
// Document Store
// =============================================================================

/// Map of slug -> live Loro document
#[derive(Default)]
pub struct DocStore {
    docs: RwLock<HashMap<String, Arc<LoroNoteDoc>>>,
}

impl DocStore {
    /// Return all loaded docs (in-memory state).
    pub async fn loaded_docs(&self) -> Vec<(String, Arc<LoroNoteDoc>)> {
        self.docs
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub async fn get_or_load(
        &self,
        slug: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<Arc<LoroNoteDoc>> {
        if let Some(doc) = self.docs.read().await.get(slug).cloned() {
            return Ok(doc);
        }

        let (frontmatter, body) = load_note_from_disk(slug, site_config).await?;
        let doc = Arc::new(LoroNoteDoc::new_with_content(frontmatter, &body)?);

        // If a .loro snapshot exists, import it
        if let Some(snapshot) = load_loro_snapshot(slug, site_config).await? {
            doc.import_snapshot(&snapshot)?;
        }

        let mut guard = self.docs.write().await;
        Ok(guard
            .entry(slug.to_string())
            .or_insert_with(|| doc.clone())
            .clone())
    }

    pub async fn snapshot(
        &self,
        slug: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<(Value, String)> {
        let doc = self.get_or_load(slug, site_config).await?;
        doc.snapshot()
    }

    pub async fn get_markdown(
        &self,
        slug: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<String> {
        let (frontmatter, body) = self.snapshot(slug, site_config).await?;
        let yaml = serde_yaml::to_string(&frontmatter)?;
        Ok(format!("---\n{}---\n{}", yaml, body))
    }

    pub async fn flush_dirty_to_disk(&self, site_config: &monowiki_core::Config) -> Result<()> {
        let docs: Vec<_> = self
            .docs
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (slug, doc) in docs {
            if doc.is_dirty() {
                let (fm, body) = doc.snapshot()?;
                write_snapshot_to_disk(&slug, &fm, &body, site_config).await?;
                write_loro_snapshot(&slug, &doc, site_config).await?;
                doc.mark_clean();
            }
        }
        Ok(())
    }

    /// Overwrite body/frontmatter for a slug and broadcast a full update to connected peers.
    pub async fn overwrite_from_plain(
        &self,
        slug: &str,
        frontmatter: Value,
        body: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<()> {
        let doc = self.get_or_load(slug, site_config).await?;
        doc.replace_body_and_frontmatter(frontmatter, body);
        Ok(())
    }
}

// =============================================================================
// Loro Document
// =============================================================================

/// A collaborative document using Loro with block structure + per-block text.
///
/// Structure uses a simple approach:
/// - "blocks" (LoroList) - List of block data as JSON strings
/// - "texts" (LoroMap) - Map of block_id -> LoroText for each block's content
/// - "frontmatter" (LoroMap) - Document metadata
pub struct LoroNoteDoc {
    doc: LoroDoc,
    /// Cached frontmatter (also stored in doc for sync)
    frontmatter: SyncRwLock<Value>,
    tx: broadcast::Sender<SyncPacket>,
    dirty: std::sync::atomic::AtomicBool,
    session_counter: std::sync::atomic::AtomicU64,
    /// ID counter for generating block IDs
    block_counter: std::sync::atomic::AtomicU64,
    /// ID counter for generating comment IDs
    comment_counter: std::sync::atomic::AtomicU64,
}

impl LoroNoteDoc {
    /// Create empty document with initialized structure
    pub fn new() -> Self {
        let doc = LoroDoc::new();
        let (tx, _) = broadcast::channel(128);

        // Initialize the structure containers
        let _ = doc.get_list("blocks");
        let _ = doc.get_map("texts");
        let _ = doc.get_map("marks");
        let _ = doc.get_map("frontmatter");
        let _ = doc.get_map("comments");

        Self {
            doc,
            frontmatter: SyncRwLock::new(Value::Object(Default::default())),
            tx,
            dirty: std::sync::atomic::AtomicBool::new(false),
            session_counter: std::sync::atomic::AtomicU64::new(1),
            block_counter: std::sync::atomic::AtomicU64::new(1),
            comment_counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Create document with initial markdown content
    pub fn new_with_content(frontmatter: Value, body: &str) -> Result<Self> {
        let this = Self::new();

        // Parse markdown into blocks and populate the structure
        this.initialize_from_markdown(body)?;

        *this.frontmatter.write() = frontmatter.clone();

        // Also store frontmatter in Loro for sync
        this.set_frontmatter_value(&frontmatter)?;

        Ok(this)
    }

    /// Generate a new unique block ID
    fn next_block_id(&self) -> String {
        let id = self
            .block_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("b{}", id)
    }

    pub fn next_session_id(&self) -> u64 {
        self.session_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    // -------------------------------------------------------------------------
    // Structure Operations
    // -------------------------------------------------------------------------

    /// Get the blocks list
    fn get_blocks_list(&self) -> LoroList {
        self.doc.get_list("blocks")
    }

    /// Get the texts map
    fn get_texts_map(&self) -> LoroMap {
        self.doc.get_map("texts")
    }

    /// Get the marks map (block_id -> JSON array of marks)
    fn get_marks_map(&self) -> LoroMap {
        self.doc.get_map("marks")
    }

    /// Get all blocks as BlockData
    pub fn get_blocks(&self) -> Vec<BlockData> {
        let blocks = self.get_blocks_list();
        let mut result = Vec::new();

        for i in 0..blocks.len() {
            if let Some(value) = blocks.get(i) {
                // ValueOrContainer - need to get inner value
                if let loro::ValueOrContainer::Value(loro_val) = value {
                    if let Some(json_str) = loro_val.as_string() {
                        if let Ok(block) = serde_json::from_str::<BlockData>(json_str) {
                            result.push(block);
                        }
                    }
                }
            }
        }
        result
    }

    /// Get all block IDs in document order
    pub fn get_block_ids(&self) -> Vec<String> {
        self.get_blocks().into_iter().map(|b| b.id).collect()
    }

    /// Insert a new block at the given position
    pub fn insert_block(&self, index: usize, meta: BlockMeta) -> Result<String> {
        let block_id = self.next_block_id();
        let blocks = self.get_blocks_list();

        let block_data = BlockData {
            id: block_id.clone(),
            kind: meta.kind.as_str().to_string(),
            attrs: meta.attrs,
            text: String::new(),
        };

        let json = serde_json::to_string(&block_data)?;
        blocks.insert(index, json)?;

        // Create text container for this block if needed
        if meta.kind.has_text() {
            let texts = self.get_texts_map();
            texts.insert(&block_id, "")?;
        }

        self.mark_dirty();
        self.broadcast_update()?;
        Ok(block_id)
    }

    /// Move a block from one position to another
    pub fn move_block(&self, from: usize, to: usize) -> Result<()> {
        let blocks = self.get_blocks_list();

        // Get the value at 'from'
        let value = blocks
            .get(from)
            .ok_or_else(|| anyhow!("Block not found at index {}", from))?;

        // Delete from old position
        blocks.delete(from, 1)?;

        // Insert at new position (adjust if needed)
        let insert_pos = if to > from { to - 1 } else { to };
        if let loro::ValueOrContainer::Value(loro_val) = value {
            if let Some(json_str) = loro_val.as_string() {
                blocks.insert(insert_pos, json_str.to_string())?;
            }
        }

        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Delete a block by index
    pub fn delete_block(&self, index: usize) -> Result<()> {
        let blocks = self.get_blocks_list();

        // Get block ID before deletion to clean up text
        if let Some(value) = blocks.get(index) {
            if let loro::ValueOrContainer::Value(loro_val) = value {
                if let Some(json_str) = loro_val.as_string() {
                    if let Ok(block) = serde_json::from_str::<BlockData>(json_str) {
                        let texts = self.get_texts_map();
                        let _ = texts.delete(&block.id);
                    }
                }
            }
        }

        blocks.delete(index, 1)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Get block metadata by ID
    pub fn get_block_meta(&self, block_id: &str) -> Option<BlockMeta> {
        let blocks = self.get_blocks();
        for block in blocks {
            if block.id == block_id {
                let kind = BlockKind::from_str(&block.kind)?;
                return Some(BlockMeta {
                    kind,
                    attrs: block.attrs,
                });
            }
        }
        None
    }

    // -------------------------------------------------------------------------
    // Text Operations (per-block text)
    // -------------------------------------------------------------------------

    /// Get text for a specific block
    pub fn get_block_text(&self, block_id: &str) -> String {
        // Try to get from the dedicated text container first
        let texts = self.get_texts_map();
        if let Some(value) = texts.get(block_id) {
            if let loro::ValueOrContainer::Value(loro_val) = value {
                if let Some(text) = loro_val.as_string() {
                    return text.to_string();
                }
            }
        }

        // Fall back to text stored in block data
        let blocks = self.get_blocks();
        for block in blocks {
            if block.id == block_id {
                return block.text;
            }
        }
        String::new()
    }

    /// Set text for a specific block
    pub fn set_block_text(&self, block_id: &str, content: &str) -> Result<()> {
        // Update in texts map
        let texts = self.get_texts_map();
        texts.insert(block_id, content)?;

        // Also update in block data for consistency
        self.update_block_text_in_list(block_id, content)?;

        self.mark_dirty();
        Ok(())
    }

    /// Update block text in the blocks list
    fn update_block_text_in_list(&self, block_id: &str, content: &str) -> Result<()> {
        let blocks = self.get_blocks_list();

        for i in 0..blocks.len() {
            if let Some(value) = blocks.get(i) {
                if let loro::ValueOrContainer::Value(loro_val) = value {
                    if let Some(json_str) = loro_val.as_string() {
                        if let Ok(mut block) = serde_json::from_str::<BlockData>(json_str) {
                            if block.id == block_id {
                                block.text = content.to_string();
                                let new_json = serde_json::to_string(&block)?;
                                blocks.delete(i, 1)?;
                                blocks.insert(i, new_json)?;
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Insert text in a block at offset
    pub fn insert_block_text(&self, block_id: &str, offset: usize, content: &str) -> Result<()> {
        let current = self.get_block_text(block_id);
        let actual_offset = offset.min(current.len());
        let mut new_text = current;
        new_text.insert_str(actual_offset, content);
        self.set_block_text(block_id, &new_text)?;

        // Adjust mark positions according to Peritext anchor semantics
        self.adjust_marks_for_insert(block_id, actual_offset, content.len())?;
        // Keep comment anchors in sync
        self.adjust_comments_for_insert(block_id, actual_offset, content.len())?;

        self.broadcast_update()?;
        Ok(())
    }

    /// Delete text in a block
    pub fn delete_block_text(&self, block_id: &str, start: usize, len: usize) -> Result<()> {
        let current = self.get_block_text(block_id);
        let mut new_text = current;
        let end = (start + len).min(new_text.len());
        let actual_len = end.saturating_sub(start);
        if start < new_text.len() {
            new_text.drain(start..end);
        }
        self.set_block_text(block_id, &new_text)?;

        // Adjust mark positions for deletion
        if actual_len > 0 {
            self.adjust_marks_for_delete(block_id, start, actual_len)?;
            self.adjust_comments_for_delete(block_id, start, actual_len)?;
        }

        self.broadcast_update()?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Mark Operations (Peritext layer)
    // -------------------------------------------------------------------------

    /// Get marks for a block from the CRDT store
    fn get_block_marks_internal(&self, block_id: &str) -> Vec<Mark> {
        let marks_map = self.get_marks_map();
        if let Some(value) = marks_map.get(block_id) {
            if let loro::ValueOrContainer::Value(loro_val) = value {
                if let Some(json_str) = loro_val.as_string() {
                    if let Ok(marks) = serde_json::from_str::<Vec<Mark>>(json_str) {
                        return marks;
                    }
                }
            }
        }
        Vec::new()
    }

    /// Save marks for a block to the CRDT store
    fn set_block_marks_internal(&self, block_id: &str, marks: &[Mark]) -> Result<()> {
        let marks_map = self.get_marks_map();
        let json_str = serde_json::to_string(marks)?;
        marks_map.insert(block_id, json_str)?;
        Ok(())
    }

    /// Add a formatting mark to a block
    ///
    /// Marks are stored with Peritext-style anchor semantics that determine
    /// how they expand/contract when text is inserted at boundaries.
    pub fn add_mark(&self, block_id: &str, mark: &Mark) -> Result<()> {
        let mut marks = self.get_block_marks_internal(block_id);

        // Check for overlapping marks of the same type and merge/replace
        marks.retain(|m| {
            // Remove if same type and overlapping
            !(m.mark_type == mark.mark_type && m.start < mark.end && m.end > mark.start)
        });

        marks.push(mark.clone());

        // Sort marks by start position for consistent ordering
        marks.sort_by_key(|m| (m.start, m.end));

        self.set_block_marks_internal(block_id, &marks)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Remove a formatting mark from a block
    ///
    /// Removes marks of the given type that overlap with the specified range.
    /// If a mark partially overlaps, it will be split or truncated.
    pub fn remove_mark(
        &self,
        block_id: &str,
        mark_type: &str,
        start: usize,
        end: usize,
    ) -> Result<()> {
        let mut marks = self.get_block_marks_internal(block_id);
        let mut new_marks = Vec::new();

        for mark in marks.drain(..) {
            if mark.mark_type != mark_type {
                // Different type, keep as-is
                new_marks.push(mark);
            } else if mark.end <= start || mark.start >= end {
                // No overlap, keep as-is
                new_marks.push(mark);
            } else if mark.start >= start && mark.end <= end {
                // Completely contained, remove (don't add to new_marks)
            } else if mark.start < start && mark.end > end {
                // Removal range is inside mark - split into two
                let left = Mark {
                    mark_type: mark.mark_type.clone(),
                    start: mark.start,
                    end: start,
                    start_anchor: mark.start_anchor.clone(),
                    end_anchor: Anchor::After,
                    attrs: mark.attrs.clone(),
                };
                let right = Mark {
                    mark_type: mark.mark_type,
                    start: end,
                    end: mark.end,
                    start_anchor: Anchor::Before,
                    end_anchor: mark.end_anchor,
                    attrs: mark.attrs,
                };
                new_marks.push(left);
                new_marks.push(right);
            } else if mark.start < start {
                // Overlaps on the left - truncate
                let truncated = Mark { end: start, ..mark };
                new_marks.push(truncated);
            } else {
                // Overlaps on the right - truncate
                let truncated = Mark { start: end, ..mark };
                new_marks.push(truncated);
            }
        }

        // Sort and save
        new_marks.sort_by_key(|m| (m.start, m.end));
        self.set_block_marks_internal(block_id, &new_marks)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Get all marks for a block
    pub fn get_marks(&self, block_id: &str) -> Vec<Mark> {
        self.get_block_marks_internal(block_id)
    }

    /// Adjust mark positions after text insertion
    ///
    /// Called internally when text is inserted to update mark boundaries
    /// according to their anchor semantics.
    pub fn adjust_marks_for_insert(&self, block_id: &str, offset: usize, len: usize) -> Result<()> {
        let mut marks = self.get_block_marks_internal(block_id);
        let mut changed = false;

        for mark in &mut marks {
            // Adjust start position
            if offset < mark.start {
                // Insertion before mark - shift mark right
                mark.start += len;
                mark.end += len;
                changed = true;
            } else if offset == mark.start {
                // Insertion at start boundary - check anchor
                match mark.start_anchor {
                    Anchor::Before => {
                        // Expand to include new text
                        mark.end += len;
                        changed = true;
                    }
                    Anchor::After => {
                        // Don't expand - shift mark right
                        mark.start += len;
                        mark.end += len;
                        changed = true;
                    }
                }
            } else if offset < mark.end {
                // Insertion inside mark - expand
                mark.end += len;
                changed = true;
            } else if offset == mark.end {
                // Insertion at end boundary - check anchor
                match mark.end_anchor {
                    Anchor::Before => {
                        // Don't expand
                    }
                    Anchor::After => {
                        // Expand to include new text
                        mark.end += len;
                        changed = true;
                    }
                }
            }
            // else: insertion after mark - no change
        }

        if changed {
            self.set_block_marks_internal(block_id, &marks)?;
        }
        Ok(())
    }

    /// Adjust mark positions after text deletion
    pub fn adjust_marks_for_delete(&self, block_id: &str, start: usize, len: usize) -> Result<()> {
        let mut marks = self.get_block_marks_internal(block_id);
        let end = start + len;
        let mut new_marks = Vec::new();

        for mut mark in marks.drain(..) {
            if mark.end <= start {
                // Mark is entirely before deletion - keep as-is
                new_marks.push(mark);
            } else if mark.start >= end {
                // Mark is entirely after deletion - shift left
                mark.start -= len;
                mark.end -= len;
                new_marks.push(mark);
            } else if mark.start >= start && mark.end <= end {
                // Mark is entirely within deletion - remove
            } else if mark.start < start && mark.end > end {
                // Deletion is inside mark - shrink
                mark.end -= len;
                new_marks.push(mark);
            } else if mark.start < start {
                // Mark overlaps on left - truncate at deletion start
                mark.end = start;
                if mark.start < mark.end {
                    new_marks.push(mark);
                }
            } else {
                // Mark overlaps on right - truncate and shift
                mark.start = start;
                mark.end -= len;
                if mark.start < mark.end {
                    new_marks.push(mark);
                }
            }
        }

        self.set_block_marks_internal(block_id, &new_marks)?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Comment Operations
    // -------------------------------------------------------------------------

    /// Generate a new unique comment ID
    fn next_comment_id(&self) -> String {
        let id = self
            .comment_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("c{}", id)
    }

    /// Get the comments map
    fn get_comments_map(&self) -> LoroMap {
        self.doc.get_map("comments")
    }

    /// Add a comment to the document
    pub fn add_comment(
        &self,
        block_id: &str,
        start: usize,
        end: usize,
        content: &str,
        author: &str,
    ) -> Result<String> {
        let comment_id = self.next_comment_id();
        let comment = Comment::new(&comment_id, block_id, start, end, content, author);

        let comments_map = self.get_comments_map();
        let json = serde_json::to_string(&comment)?;
        comments_map.insert(&comment_id, json)?;

        self.mark_dirty();
        self.broadcast_update()?;
        Ok(comment_id)
    }

    /// Add a reply to an existing comment
    pub fn add_comment_reply(
        &self,
        parent_id: &str,
        content: &str,
        author: &str,
    ) -> Result<String> {
        let parent = self
            .get_comment(parent_id)?
            .ok_or_else(|| anyhow!("Parent comment not found: {}", parent_id))?;

        let comment_id = self.next_comment_id();
        let reply = Comment::reply(&comment_id, &parent, content, author);

        let comments_map = self.get_comments_map();
        let json = serde_json::to_string(&reply)?;
        comments_map.insert(&comment_id, json)?;

        self.mark_dirty();
        self.broadcast_update()?;
        Ok(comment_id)
    }

    /// Get a comment by ID
    pub fn get_comment(&self, comment_id: &str) -> Result<Option<Comment>> {
        let comments_map = self.get_comments_map();
        if let Some(value) = comments_map.get(comment_id) {
            if let loro::ValueOrContainer::Value(loro_val) = value {
                if let Some(json_str) = loro_val.as_string() {
                    let comment: Comment = serde_json::from_str(json_str)?;
                    return Ok(Some(comment));
                }
            }
        }
        Ok(None)
    }

    /// Get all comments on the document
    pub fn get_all_comments(&self) -> Vec<Comment> {
        let comments_map = self.get_comments_map();
        let mut comments = Vec::new();

        // Iterate through all keys in the map
        for key in comments_map.keys() {
            let key_str = key.as_str();
            if let Some(value) = comments_map.get(key_str) {
                if let loro::ValueOrContainer::Value(loro_val) = value {
                    if let Some(json_str) = loro_val.as_string() {
                        if let Ok(comment) = serde_json::from_str::<Comment>(json_str) {
                            comments.push(comment);
                        }
                    }
                }
            }
        }

        // Sort by creation time
        comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        comments
    }

    /// Get comments for a specific block
    pub fn get_block_comments(&self, block_id: &str) -> Vec<Comment> {
        self.get_all_comments()
            .into_iter()
            .filter(|c| c.block_id == block_id)
            .collect()
    }

    /// Get unresolved comments
    pub fn get_unresolved_comments(&self) -> Vec<Comment> {
        self.get_all_comments()
            .into_iter()
            .filter(|c| !c.resolved)
            .collect()
    }

    /// Resolve a comment
    pub fn resolve_comment(&self, comment_id: &str) -> Result<()> {
        let comments_map = self.get_comments_map();

        if let Some(mut comment) = self.get_comment(comment_id)? {
            comment.resolved = true;
            let json = serde_json::to_string(&comment)?;
            comments_map.insert(comment_id, json)?;
            self.mark_dirty();
            self.broadcast_update()?;
            Ok(())
        } else {
            Err(anyhow!("Comment not found: {}", comment_id))
        }
    }

    /// Delete a comment
    pub fn delete_comment(&self, comment_id: &str) -> Result<()> {
        let comments_map = self.get_comments_map();
        comments_map.delete(comment_id)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Update comment positions when text is inserted
    /// Called after insert_block_text to keep comments anchored correctly
    pub fn adjust_comments_for_insert(
        &self,
        block_id: &str,
        offset: usize,
        len: usize,
    ) -> Result<()> {
        let comments_map = self.get_comments_map();
        let block_comments: Vec<_> = self.get_block_comments(block_id);

        for mut comment in block_comments {
            let mut changed = false;

            if offset <= comment.start {
                // Insertion before comment - shift right
                comment.start += len;
                comment.end += len;
                changed = true;
            } else if offset < comment.end {
                // Insertion inside comment - expand
                comment.end += len;
                changed = true;
            }

            if changed {
                let json = serde_json::to_string(&comment)?;
                comments_map.insert(&comment.id, json)?;
            }
        }

        Ok(())
    }

    /// Update comment positions when text is deleted
    pub fn adjust_comments_for_delete(
        &self,
        block_id: &str,
        start: usize,
        len: usize,
    ) -> Result<()> {
        let comments_map = self.get_comments_map();
        let end = start + len;
        let block_comments: Vec<_> = self.get_block_comments(block_id);

        for mut comment in block_comments {
            if comment.end <= start {
                // Comment before deletion - no change
                continue;
            } else if comment.start >= end {
                // Comment after deletion - shift left
                comment.start -= len;
                comment.end -= len;
                let json = serde_json::to_string(&comment)?;
                comments_map.insert(&comment.id, json)?;
            } else if comment.start >= start && comment.end <= end {
                // Comment entirely within deletion - delete the comment
                comments_map.delete(&comment.id)?;
            } else if comment.start < start && comment.end > end {
                // Deletion inside comment - shrink
                comment.end -= len;
                let json = serde_json::to_string(&comment)?;
                comments_map.insert(&comment.id, json)?;
            } else if comment.start < start {
                // Comment overlaps left side - truncate
                comment.end = start;
                if comment.start < comment.end {
                    let json = serde_json::to_string(&comment)?;
                    comments_map.insert(&comment.id, json)?;
                } else {
                    comments_map.delete(&comment.id)?;
                }
            } else {
                // Comment overlaps right side - shift and truncate
                comment.start = start;
                comment.end = comment.end - len;
                if comment.start < comment.end {
                    let json = serde_json::to_string(&comment)?;
                    comments_map.insert(&comment.id, json)?;
                } else {
                    comments_map.delete(&comment.id)?;
                }
            }
        }

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Frontmatter Operations
    // -------------------------------------------------------------------------

    fn set_frontmatter_value(&self, value: &Value) -> Result<()> {
        let fm_map = self.doc.get_map("frontmatter");
        let json_str = serde_json::to_string(value)?;
        fm_map.insert("data", json_str)?;
        Ok(())
    }

    pub fn get_frontmatter(&self) -> Value {
        self.frontmatter.read().clone()
    }

    // -------------------------------------------------------------------------
    // Document Conversion
    // -------------------------------------------------------------------------

    /// Initialize document structure from markdown
    fn initialize_from_markdown(&self, body: &str) -> Result<()> {
        let parsed_blocks = parse_markdown_to_blocks(body);
        let blocks = self.get_blocks_list();
        let texts = self.get_texts_map();

        for (kind, text, attrs) in parsed_blocks {
            let block_id = self.next_block_id();

            let block_data = BlockData {
                id: block_id.clone(),
                kind: kind.as_str().to_string(),
                attrs,
                text: text.clone(),
            };

            let json = serde_json::to_string(&block_data)?;
            blocks.push(json)?;

            // Also store text in texts map for efficient access
            if kind.has_text() && !text.is_empty() {
                texts.insert(&block_id, text)?;
            }
        }

        Ok(())
    }

    /// Export document to markdown
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();
        let blocks = self.get_blocks();

        for block in blocks {
            let kind = BlockKind::from_str(&block.kind).unwrap_or(BlockKind::Paragraph);
            let text = if block.text.is_empty() {
                self.get_block_text(&block.id)
            } else {
                block.text.clone()
            };

            match kind {
                BlockKind::Heading => {
                    let level = block
                        .attrs
                        .get("level")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1) as usize;
                    let prefix = "#".repeat(level);
                    output.push_str(&format!("{} {}\n\n", prefix, text));
                }
                BlockKind::Paragraph => {
                    output.push_str(&text);
                    output.push_str("\n\n");
                }
                BlockKind::CodeBlock => {
                    let lang = block
                        .attrs
                        .get("language")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    output.push_str(&format!("```{}\n{}\n```\n\n", lang, text));
                }
                BlockKind::BulletList | BlockKind::OrderedList => {
                    // Lists are containers, text is in ListItems
                }
                BlockKind::ListItem => {
                    output.push_str(&format!("- {}\n", text));
                }
                BlockKind::Blockquote => {
                    for line in text.lines() {
                        output.push_str(&format!("> {}\n", line));
                    }
                    output.push('\n');
                }
                BlockKind::ThematicBreak => {
                    output.push_str("---\n\n");
                }
                BlockKind::MathBlock => {
                    output.push_str(&format!("$$\n{}\n$$\n\n", text));
                }
                _ => {
                    if !text.is_empty() {
                        output.push_str(&text);
                        output.push_str("\n\n");
                    }
                }
            }
        }

        output.trim_end().to_string()
    }

    // -------------------------------------------------------------------------
    // Legacy API (for backward compatibility)
    // -------------------------------------------------------------------------

    /// Get full body text (legacy: concatenates all blocks)
    pub fn get_body_text(&self) -> String {
        self.to_markdown()
    }

    /// Insert text at offset in flattened body (legacy)
    pub fn insert_text(&self, offset: usize, content: &str) -> Result<()> {
        let text = self.doc.get_text("body");
        text.insert(offset, content)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Delete text at offset in flattened body (legacy)
    pub fn delete_text(&self, start: usize, len: usize) -> Result<()> {
        let text = self.doc.get_text("body");
        text.delete(start, len)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Sync Operations
    // -------------------------------------------------------------------------

    /// Export full state for sync
    pub fn export_snapshot(&self) -> Result<Vec<u8>> {
        self.doc
            .export(ExportMode::Snapshot)
            .map_err(|e| anyhow!("Export error: {:?}", e))
    }

    /// Import snapshot from another peer
    pub fn import_snapshot(&self, data: &[u8]) -> Result<()> {
        self.doc.import(data)?;
        self.mark_dirty();
        Ok(())
    }

    /// Export updates for sync
    pub fn export_updates(&self) -> Result<Vec<u8>> {
        self.export_snapshot()
    }

    /// Apply updates from another peer
    pub fn apply_updates(&self, data: &[u8]) -> Result<()> {
        self.doc.import(data)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Get current version (for sync protocol)
    pub fn version(&self) -> Vec<u8> {
        self.doc.oplog_vv().encode()
    }

    /// Replace body/frontmatter and emit a full update for connected peers.
    pub fn replace_body_and_frontmatter(&self, frontmatter: Value, body: &str) {
        {
            let mut guard = self.frontmatter.write();
            *guard = frontmatter.clone();
        }

        let _ = self.set_frontmatter_value(&frontmatter);
        let _ = self.reinitialize_body(body);

        self.mark_dirty();

        if let Ok(update) = self.export_snapshot() {
            let _ = self.broadcast(update, 0);
        }
    }

    fn reinitialize_body(&self, body: &str) -> Result<()> {
        // Clear existing blocks
        let blocks = self.get_blocks_list();
        let len = blocks.len();
        if len > 0 {
            blocks.delete(0, len)?;
        }

        // Clear texts
        let texts = self.get_texts_map();
        // Note: Can't easily clear a map, but it will be overwritten

        // Reset block counter
        self.block_counter
            .store(1, std::sync::atomic::Ordering::Relaxed);

        // Re-parse markdown
        self.initialize_from_markdown(body)?;
        Ok(())
    }

    /// Snapshot to frontmatter + body string
    pub fn snapshot(&self) -> Result<(Value, String)> {
        let fm = self.frontmatter.read().clone();
        let body = self.to_markdown();
        Ok((fm, body))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SyncPacket> {
        self.tx.subscribe()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn mark_dirty(&self) {
        self.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn mark_clean(&self) {
        self.dirty
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn broadcast(&self, payload: Vec<u8>, sender_id: u64) {
        let _ = self.tx.send(SyncPacket { sender_id, payload });
    }

    fn broadcast_update(&self) -> Result<()> {
        let update = self.export_snapshot()?;
        let _ = self.tx.send(SyncPacket {
            payload: update,
            sender_id: 0,
        });
        Ok(())
    }
}

impl Default for LoroNoteDoc {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct SyncPacket {
    pub sender_id: u64,
    pub payload: Vec<u8>,
}

// =============================================================================
// Markdown Parsing Helpers
// =============================================================================

/// Parse markdown into blocks (simple heuristic-based parser)
fn parse_markdown_to_blocks(body: &str) -> Vec<(BlockKind, String, HashMap<String, Value>)> {
    let mut blocks = Vec::new();
    let mut lines = body.lines().peekable();

    while let Some(line) = lines.next() {
        // Heading
        if line.starts_with('#') {
            let level = line.chars().take_while(|&c| c == '#').count();
            let text = line[level..].trim().to_string();
            let mut attrs = HashMap::new();
            attrs.insert("level".to_string(), Value::Number(level.into()));
            blocks.push((BlockKind::Heading, text, attrs));
            continue;
        }

        // Code block
        if line.starts_with("```") || line.starts_with("~~~") {
            let fence = if line.starts_with("```") {
                "```"
            } else {
                "~~~"
            };
            let lang = line[fence.len()..].trim().to_string();
            let mut code_lines = Vec::new();

            while let Some(code_line) = lines.next() {
                if code_line.starts_with(fence) {
                    break;
                }
                code_lines.push(code_line);
            }

            let text = code_lines.join("\n");
            let mut attrs = HashMap::new();
            if !lang.is_empty() {
                attrs.insert("language".to_string(), Value::String(lang));
            }
            blocks.push((BlockKind::CodeBlock, text, attrs));
            continue;
        }

        // Math block
        if line.starts_with("$$") {
            let mut math_lines = Vec::new();
            while let Some(math_line) = lines.next() {
                if math_line.starts_with("$$") {
                    break;
                }
                math_lines.push(math_line);
            }
            let text = math_lines.join("\n");
            blocks.push((BlockKind::MathBlock, text, HashMap::new()));
            continue;
        }

        // Thematic break
        if line.trim() == "---" || line.trim() == "***" || line.trim() == "___" {
            blocks.push((BlockKind::ThematicBreak, String::new(), HashMap::new()));
            continue;
        }

        // Blockquote
        if line.starts_with('>') {
            let mut quote_lines = vec![line[1..].trim().to_string()];
            while let Some(&next_line) = lines.peek() {
                if next_line.starts_with('>') {
                    quote_lines.push(next_line[1..].trim().to_string());
                    lines.next();
                } else {
                    break;
                }
            }
            let text = quote_lines.join("\n");
            blocks.push((BlockKind::Blockquote, text, HashMap::new()));
            continue;
        }

        // List item
        if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
            let text = line[2..].to_string();
            blocks.push((BlockKind::ListItem, text, HashMap::new()));
            continue;
        }

        // Ordered list item (handles multi-digit numbers like "10. item")
        if let Some(dot_pos) = line.find(". ") {
            let prefix = &line[..dot_pos];
            if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()) {
                let text = line[dot_pos + 2..].to_string();
                blocks.push((BlockKind::ListItem, text, HashMap::new()));
                continue;
            }
        }

        // Empty line - skip
        if line.trim().is_empty() {
            continue;
        }

        // Default: paragraph
        let mut para_lines = vec![line.to_string()];
        while let Some(&next_line) = lines.peek() {
            // Check for block-level element starts that should break the paragraph
            let trimmed = next_line.trim();
            let is_thematic_break = trimmed == "---" || trimmed == "***" || trimmed == "___";
            let is_unordered_list = next_line.starts_with("- ")
                || next_line.starts_with("* ")
                || next_line.starts_with("+ ");
            let is_ordered_list = next_line
                .find(". ")
                .map(|dot_pos| {
                    let prefix = &next_line[..dot_pos];
                    !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit())
                })
                .unwrap_or(false);

            if next_line.trim().is_empty()
                || next_line.starts_with('#')
                || next_line.starts_with("```")
                || next_line.starts_with("~~~")
                || next_line.starts_with("$$")
                || next_line.starts_with('>')
                || is_unordered_list
                || is_ordered_list
                || is_thematic_break
            {
                break;
            }
            para_lines.push(next_line.to_string());
            lines.next();
        }
        let text = para_lines.join("\n");
        blocks.push((BlockKind::Paragraph, text, HashMap::new()));
    }

    blocks
}

// =============================================================================
// Disk I/O Helpers
// =============================================================================

/// Convert a slug to a vault-relative path, rejecting traversal.
pub fn slug_to_rel(slug: &str) -> Result<PathBuf> {
    use std::path::Component;

    let candidate = PathBuf::from(slug.trim_matches('/'));
    let mut clean = PathBuf::new();
    for comp in candidate.components() {
        match comp {
            Component::CurDir => continue,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("invalid path component in slug"))
            }
            Component::Normal(s) => clean.push(s),
        }
    }

    if clean.as_os_str().is_empty() {
        return Err(anyhow!("empty slug"));
    }

    if clean.extension().is_none() {
        clean.set_extension("md");
    }

    Ok(clean)
}

async fn load_note_from_disk(
    slug: &str,
    config: &monowiki_core::Config,
) -> Result<(Value, String)> {
    let path = config.vault_dir().join(slug_to_rel(slug)?);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => {
            let (fm, body) = monowiki_core::frontmatter::parse_frontmatter(&content)?;
            Ok((serde_json::to_value(fm)?, body))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Create an empty document when the file doesn't exist yet.
            let fm = serde_json::json!({
                "title": slug,
            });
            Ok((fm, String::new()))
        }
        Err(e) => Err(e.into()),
    }
}

async fn load_loro_snapshot(slug: &str, config: &monowiki_core::Config) -> Result<Option<Vec<u8>>> {
    let mut path = PathBuf::from(".collab").join(slug_to_rel(slug)?);
    path.set_extension("loro");
    let full = config.vault_dir().join(&path);
    match tokio::fs::read(&full).await {
        Ok(data) => Ok(Some(data)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

async fn write_snapshot_to_disk(
    slug: &str,
    fm: &Value,
    body: &str,
    config: &monowiki_core::Config,
) -> Result<()> {
    let path = config.vault_dir().join(slug_to_rel(slug)?);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let yaml = serde_yaml::to_string(fm)?;
    let content = format!("---\n{}---\n{}", yaml, body);
    tokio::fs::write(&path, content).await?;
    Ok(())
}

async fn write_loro_snapshot(
    slug: &str,
    doc: &LoroNoteDoc,
    config: &monowiki_core::Config,
) -> Result<()> {
    let mut path = PathBuf::from(".collab").join(slug_to_rel(slug)?);
    path.set_extension("loro");
    let full = config.vault_dir().join(&path);
    if let Some(parent) = full.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&full, doc.export_snapshot()?).await?;
    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_kind_roundtrip() {
        for kind in [
            BlockKind::Section,
            BlockKind::Heading,
            BlockKind::Paragraph,
            BlockKind::CodeBlock,
            BlockKind::BulletList,
            BlockKind::ListItem,
        ] {
            assert_eq!(BlockKind::from_str(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn test_parse_markdown_headings() {
        let md = "# Title\n\n## Section\n\nParagraph text.";
        let blocks = parse_markdown_to_blocks(md);

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].0, BlockKind::Heading);
        assert_eq!(blocks[0].1, "Title");
        assert_eq!(blocks[0].2.get("level").and_then(|v| v.as_u64()), Some(1));

        assert_eq!(blocks[1].0, BlockKind::Heading);
        assert_eq!(blocks[1].1, "Section");
        assert_eq!(blocks[1].2.get("level").and_then(|v| v.as_u64()), Some(2));

        assert_eq!(blocks[2].0, BlockKind::Paragraph);
        assert_eq!(blocks[2].1, "Paragraph text.");
    }

    #[test]
    fn test_parse_markdown_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let blocks = parse_markdown_to_blocks(md);

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, BlockKind::CodeBlock);
        assert_eq!(blocks[0].1, "fn main() {}");
        assert_eq!(
            blocks[0].2.get("language").and_then(|v| v.as_str()),
            Some("rust")
        );
    }

    #[test]
    fn test_parse_markdown_list() {
        let md = "- Item 1\n- Item 2\n- Item 3";
        let blocks = parse_markdown_to_blocks(md);

        assert_eq!(blocks.len(), 3);
        for (i, block) in blocks.iter().enumerate() {
            assert_eq!(block.0, BlockKind::ListItem);
            assert_eq!(block.1, format!("Item {}", i + 1));
        }
    }

    #[test]
    fn test_parse_markdown_ordered_list_multidigit() {
        // Test that multi-digit ordered lists work (e.g., "10. item")
        let md = "1. First\n10. Tenth\n99. Ninety-ninth";
        let blocks = parse_markdown_to_blocks(md);

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].0, BlockKind::ListItem);
        assert_eq!(blocks[0].1, "First");
        assert_eq!(blocks[1].0, BlockKind::ListItem);
        assert_eq!(blocks[1].1, "Tenth");
        assert_eq!(blocks[2].0, BlockKind::ListItem);
        assert_eq!(blocks[2].1, "Ninety-ninth");
    }

    #[test]
    fn test_parse_markdown_all_list_markers() {
        // Test all unordered list markers (* and +)
        let md = "* Star item\n+ Plus item\n- Dash item";
        let blocks = parse_markdown_to_blocks(md);

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].0, BlockKind::ListItem);
        assert_eq!(blocks[0].1, "Star item");
        assert_eq!(blocks[1].0, BlockKind::ListItem);
        assert_eq!(blocks[1].1, "Plus item");
        assert_eq!(blocks[2].0, BlockKind::ListItem);
        assert_eq!(blocks[2].1, "Dash item");
    }

    #[test]
    fn test_parse_markdown_paragraph_breaks() {
        // Test that paragraphs break correctly on all block types
        let md = "Paragraph one\n$$\nx^2\n$$\nParagraph two\n---\nParagraph three\n* List item";
        let blocks = parse_markdown_to_blocks(md);

        assert_eq!(blocks.len(), 6);
        assert_eq!(blocks[0].0, BlockKind::Paragraph);
        assert_eq!(blocks[0].1, "Paragraph one");
        assert_eq!(blocks[1].0, BlockKind::MathBlock);
        assert_eq!(blocks[1].1, "x^2");
        assert_eq!(blocks[2].0, BlockKind::Paragraph);
        assert_eq!(blocks[2].1, "Paragraph two");
        assert_eq!(blocks[3].0, BlockKind::ThematicBreak);
        assert_eq!(blocks[4].0, BlockKind::Paragraph);
        assert_eq!(blocks[4].1, "Paragraph three");
        assert_eq!(blocks[5].0, BlockKind::ListItem);
        assert_eq!(blocks[5].1, "List item");
    }

    #[test]
    fn test_parse_markdown_blockquote_break() {
        // Test that blockquotes break paragraphs even without space after >
        let md = "Paragraph\n> Quote text";
        let blocks = parse_markdown_to_blocks(md);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, BlockKind::Paragraph);
        assert_eq!(blocks[0].1, "Paragraph");
        assert_eq!(blocks[1].0, BlockKind::Blockquote);
    }

    #[test]
    fn test_mark_creation() {
        let mark = Mark::new("emphasis", 0, 10)
            .with_anchor(Anchor::Before, Anchor::After)
            .with_attr("weight", Value::String("bold".to_string()));

        assert_eq!(mark.mark_type, "emphasis");
        assert_eq!(mark.start, 0);
        assert_eq!(mark.end, 10);
        assert_eq!(mark.start_anchor, Anchor::Before);
        assert_eq!(mark.end_anchor, Anchor::After);
        assert_eq!(
            mark.attrs.get("weight").and_then(|v| v.as_str()),
            Some("bold")
        );
    }

    #[test]
    fn test_loro_doc_creation() {
        let doc = LoroNoteDoc::new();
        assert!(!doc.is_dirty());

        let block_ids = doc.get_block_ids();
        assert!(block_ids.is_empty());
    }

    #[test]
    fn test_loro_doc_with_content() {
        let md = "# Hello\n\nWorld";
        let doc = LoroNoteDoc::new_with_content(Value::Object(Default::default()), md).unwrap();

        let block_ids = doc.get_block_ids();
        assert_eq!(block_ids.len(), 2);

        // Check first block is heading
        let meta = doc.get_block_meta(&block_ids[0]).unwrap();
        assert_eq!(meta.kind, BlockKind::Heading);
        assert_eq!(doc.get_block_text(&block_ids[0]), "Hello");

        // Check second block is paragraph
        let meta = doc.get_block_meta(&block_ids[1]).unwrap();
        assert_eq!(meta.kind, BlockKind::Paragraph);
        assert_eq!(doc.get_block_text(&block_ids[1]), "World");
    }

    #[test]
    fn test_loro_doc_roundtrip() {
        let original =
            "# Title\n\nParagraph one.\n\n```python\nprint('hello')\n```\n\n- Item 1\n- Item 2";
        let doc =
            LoroNoteDoc::new_with_content(Value::Object(Default::default()), original).unwrap();

        let exported = doc.to_markdown();

        // Verify key content is preserved
        assert!(exported.contains("# Title"));
        assert!(exported.contains("Paragraph one."));
        assert!(exported.contains("```python"));
        assert!(exported.contains("print('hello')"));
        assert!(exported.contains("- Item 1"));
        assert!(exported.contains("- Item 2"));
    }

    #[test]
    fn test_block_text_operations() {
        let doc =
            LoroNoteDoc::new_with_content(Value::Object(Default::default()), "# Test\n\nHello")
                .unwrap();

        let block_ids = doc.get_block_ids();
        let para_id = &block_ids[1];

        // Insert text
        doc.insert_block_text(para_id, 5, " World").unwrap();
        assert_eq!(doc.get_block_text(para_id), "Hello World");

        // Delete text
        doc.delete_block_text(para_id, 5, 6).unwrap();
        assert_eq!(doc.get_block_text(para_id), "Hello");
    }

    #[test]
    fn test_add_and_get_marks() {
        let doc = LoroNoteDoc::new_with_content(
            Value::Object(Default::default()),
            "# Test\n\nHello World",
        )
        .unwrap();

        let block_ids = doc.get_block_ids();
        let para_id = &block_ids[1];

        // Add a bold mark
        let mark = Mark::new("strong", 0, 5);
        doc.add_mark(para_id, &mark).unwrap();

        let marks = doc.get_marks(para_id);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].mark_type, "strong");
        assert_eq!(marks[0].start, 0);
        assert_eq!(marks[0].end, 5);
    }

    #[test]
    fn test_remove_mark() {
        let doc = LoroNoteDoc::new_with_content(
            Value::Object(Default::default()),
            "# Test\n\nHello World",
        )
        .unwrap();

        let block_ids = doc.get_block_ids();
        let para_id = &block_ids[1];

        // Add marks
        doc.add_mark(para_id, &Mark::new("strong", 0, 5)).unwrap();
        doc.add_mark(para_id, &Mark::new("emphasis", 6, 11))
            .unwrap();

        // Remove the strong mark
        doc.remove_mark(para_id, "strong", 0, 5).unwrap();

        let marks = doc.get_marks(para_id);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].mark_type, "emphasis");
    }

    #[test]
    fn test_mark_split_on_partial_remove() {
        let doc = LoroNoteDoc::new_with_content(
            Value::Object(Default::default()),
            "# Test\n\nHello World Test",
        )
        .unwrap();

        let block_ids = doc.get_block_ids();
        let para_id = &block_ids[1];

        // Add a mark spanning the whole text
        doc.add_mark(para_id, &Mark::new("strong", 0, 16)).unwrap();

        // Remove from middle - should split
        doc.remove_mark(para_id, "strong", 6, 11).unwrap();

        let marks = doc.get_marks(para_id);
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].start, 0);
        assert_eq!(marks[0].end, 6);
        assert_eq!(marks[1].start, 11);
        assert_eq!(marks[1].end, 16);
    }

    #[test]
    fn test_marks_adjust_on_insert() {
        let doc =
            LoroNoteDoc::new_with_content(Value::Object(Default::default()), "# Test\n\nHello")
                .unwrap();

        let block_ids = doc.get_block_ids();
        let para_id = &block_ids[1];

        // Add a mark on "ello" (positions 1-5)
        doc.add_mark(para_id, &Mark::new("strong", 1, 5)).unwrap();

        // Insert " World" at position 5 (end of "Hello")
        doc.insert_block_text(para_id, 5, " World").unwrap();

        // Mark should expand to include " World" due to Anchor::After at end
        let marks = doc.get_marks(para_id);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].start, 1);
        assert_eq!(marks[0].end, 11); // 5 + 6 = 11
    }

    #[test]
    fn test_marks_adjust_on_delete() {
        let doc = LoroNoteDoc::new_with_content(
            Value::Object(Default::default()),
            "# Test\n\nHello World",
        )
        .unwrap();

        let block_ids = doc.get_block_ids();
        let para_id = &block_ids[1];

        // Add a mark on "World" (positions 6-11)
        doc.add_mark(para_id, &Mark::new("strong", 6, 11)).unwrap();

        // Delete "Hello " (positions 0-6)
        doc.delete_block_text(para_id, 0, 6).unwrap();

        // Mark should shift left
        let marks = doc.get_marks(para_id);
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].start, 0);
        assert_eq!(marks[0].end, 5);
    }
}
