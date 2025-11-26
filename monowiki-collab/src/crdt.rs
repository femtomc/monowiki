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
        doc.replace_body_and_frontmatter(frontmatter, body).await;
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
    frontmatter: RwLock<Value>,
    tx: broadcast::Sender<SyncPacket>,
    dirty: std::sync::atomic::AtomicBool,
    session_counter: std::sync::atomic::AtomicU64,
    /// ID counter for generating block IDs
    block_counter: std::sync::atomic::AtomicU64,
}

impl LoroNoteDoc {
    /// Create empty document with initialized structure
    pub fn new() -> Self {
        let doc = LoroDoc::new();
        let (tx, _) = broadcast::channel(128);

        // Initialize the structure containers
        let _ = doc.get_list("blocks");
        let _ = doc.get_map("texts");
        let _ = doc.get_map("frontmatter");

        Self {
            doc,
            frontmatter: RwLock::new(Value::Object(Default::default())),
            tx,
            dirty: std::sync::atomic::AtomicBool::new(false),
            session_counter: std::sync::atomic::AtomicU64::new(1),
            block_counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Create document with initial markdown content
    pub fn new_with_content(frontmatter: Value, body: &str) -> Result<Self> {
        let this = Self::new();

        // Parse markdown into blocks and populate the structure
        this.initialize_from_markdown(body)?;

        *this.frontmatter.blocking_write() = frontmatter.clone();

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
        let value = blocks.get(from).ok_or_else(|| anyhow!("Block not found at index {}", from))?;

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
        let mut new_text = current;
        new_text.insert_str(offset.min(new_text.len()), content);
        self.set_block_text(block_id, &new_text)?;
        self.broadcast_update()?;
        Ok(())
    }

    /// Delete text in a block
    pub fn delete_block_text(&self, block_id: &str, start: usize, len: usize) -> Result<()> {
        let current = self.get_block_text(block_id);
        let mut new_text = current;
        let end = (start + len).min(new_text.len());
        if start < new_text.len() {
            new_text.drain(start..end);
        }
        self.set_block_text(block_id, &new_text)?;
        self.broadcast_update()?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Mark Operations (Peritext layer)
    // -------------------------------------------------------------------------

    /// Add a formatting mark to a block
    pub fn add_mark(&self, _block_id: &str, _mark: &Mark) -> Result<()> {
        // TODO: Implement proper mark support using Loro's rich text marks
        // For now, marks are not persisted
        self.mark_dirty();
        Ok(())
    }

    /// Remove a formatting mark from a block
    pub fn remove_mark(&self, _block_id: &str, _mark_type: &str, _start: usize, _end: usize) -> Result<()> {
        // TODO: Implement proper mark support
        self.mark_dirty();
        Ok(())
    }

    /// Get all marks for a block
    pub fn get_marks(&self, _block_id: &str) -> Vec<Mark> {
        // TODO: Implement proper mark support
        Vec::new()
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

    pub async fn get_frontmatter(&self) -> Value {
        self.frontmatter.read().await.clone()
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
    pub async fn replace_body_and_frontmatter(&self, frontmatter: Value, body: &str) {
        {
            let mut guard = self.frontmatter.write().await;
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
        self.block_counter.store(1, std::sync::atomic::Ordering::Relaxed);

        // Re-parse markdown
        self.initialize_from_markdown(body)?;
        Ok(())
    }

    /// Snapshot to frontmatter + body string
    pub fn snapshot(&self) -> Result<(Value, String)> {
        let fm = self.frontmatter.blocking_read().clone();
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
        self.dirty.store(false, std::sync::atomic::Ordering::Relaxed);
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
            let fence = if line.starts_with("```") { "```" } else { "~~~" };
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

        // Ordered list item
        if let Some(rest) = line.strip_prefix(|c: char| c.is_ascii_digit()) {
            if rest.starts_with(". ") {
                let text = rest[2..].to_string();
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
            if next_line.trim().is_empty()
                || next_line.starts_with('#')
                || next_line.starts_with("```")
                || next_line.starts_with("~~~")
                || next_line.starts_with("- ")
                || next_line.starts_with("> ")
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

async fn load_note_from_disk(slug: &str, config: &monowiki_core::Config) -> Result<(Value, String)> {
    let path = config.vault_dir().join(slug_to_rel(slug)?);
    let content = tokio::fs::read_to_string(&path).await?;
    let (fm, body) = monowiki_core::frontmatter::parse_frontmatter(&content)?;
    Ok((serde_json::to_value(fm)?, body))
}

async fn load_loro_snapshot(
    slug: &str,
    config: &monowiki_core::Config,
) -> Result<Option<Vec<u8>>> {
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
        let original = "# Title\n\nParagraph one.\n\n```python\nprint('hello')\n```\n\n- Item 1\n- Item 2";
        let doc = LoroNoteDoc::new_with_content(Value::Object(Default::default()), original).unwrap();

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
        let doc = LoroNoteDoc::new_with_content(
            Value::Object(Default::default()),
            "# Test\n\nHello",
        )
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
}
