//! Layout computation queries
//!
//! These queries compute layout information for rendering.
//! Provides both document-level and block-level layout for fine-grained invalidation.

use crate::durability::Durability;
use crate::queries::expand::{ExpandBlockQuery, ExpandToContentQuery};
use crate::queries::source::{BlockId, DocId};
use crate::query::{Query, QueryDatabase};

/// Viewport dimensions for layout
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    pub fn new(width: u32, height: u32) -> Self {
        Viewport { width, height }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Viewport {
            width: 800,
            height: 600,
        }
    }
}

/// Query for active style configuration
pub struct ActiveStylesQuery;

impl Query for ActiveStylesQuery {
    type Key = ();
    type Value = StyleConfig;

    fn execute<DB: QueryDatabase>(_db: &DB, _key: &Self::Key) -> Self::Value {
        StyleConfig::default()
    }

    fn durability() -> Durability {
        Durability::Durable
    }

    fn name() -> &'static str {
        "ActiveStylesQuery"
    }
}

/// Style configuration
#[derive(Debug, Clone, PartialEq)]
pub struct StyleConfig {
    pub font_family: String,
    pub font_size: u32,
    pub line_height: f32,
    pub max_width: Option<u32>,
}

impl Eq for StyleConfig {}

impl std::hash::Hash for StyleConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.font_family.hash(state);
        self.font_size.hash(state);
        self.line_height.to_bits().hash(state);
        self.max_width.hash(state);
    }
}

impl Default for StyleConfig {
    fn default() -> Self {
        StyleConfig {
            font_family: "sans-serif".to_string(),
            font_size: 16,
            line_height: 1.5,
            max_width: Some(800),
        }
    }
}

/// Query for computing layout of a document
pub struct LayoutDocumentQuery;

impl Query for LayoutDocumentQuery {
    type Key = (DocId, Viewport);
    type Value = Layout;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        let (doc_id, viewport) = key;

        // Get expanded content (creates dependency)
        let expand_result = db.query::<ExpandToContentQuery>(doc_id.clone());

        // Get styles (creates dependency)
        let styles = db.query::<ActiveStylesQuery>(());

        // Compute layout if we have content
        match expand_result.content {
            Some(text) => compute_text_layout(&text, &styles, viewport),
            None => Layout::new(),
        }
    }

    fn durability() -> Durability {
        Durability::Session
    }

    fn name() -> &'static str {
        "LayoutDocumentQuery"
    }
}

/// Query for computing layout of a single block
pub struct LayoutBlockQuery;

impl Query for LayoutBlockQuery {
    type Key = (BlockId, Viewport);
    type Value = Layout;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        let (block_id, viewport) = key;

        // Get expanded content for this block (creates dependency)
        let expand_result = db.query::<ExpandBlockQuery>(block_id.clone());

        // Get styles (creates dependency)
        let styles = db.query::<ActiveStylesQuery>(());

        // Compute layout if we have content
        match expand_result.content {
            Some(text) => compute_text_layout(&text, &styles, viewport),
            None => Layout::new(),
        }
    }

    fn durability() -> Durability {
        Durability::Session
    }

    fn name() -> &'static str {
        "LayoutBlockQuery"
    }
}

/// Layout information for rendering
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Layout {
    pub boxes: Vec<LayoutBox>,
    pub total_height: u32,
}

impl Layout {
    pub fn new() -> Self {
        Layout {
            boxes: Vec::new(),
            total_height: 0,
        }
    }

    pub fn push(&mut self, layout_box: LayoutBox) {
        self.total_height = self.total_height.max(layout_box.y + layout_box.height);
        self.boxes.push(layout_box);
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

/// A positioned layout box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayoutBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub kind: LayoutKind,
}

/// Kind of layout box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LayoutKind {
    Heading { level: usize, text: String },
    Paragraph { text: String },
    CodeBlock { lang: Option<String>, code: String },
    List { items: Vec<String> },
    Blockquote { text: String },
    ThematicBreak,
}

/// Compute layout from plain text
fn compute_text_layout(text: &str, styles: &StyleConfig, viewport: &Viewport) -> Layout {
    let mut layout = Layout::new();
    let mut y = 0u32;

    let width = styles
        .max_width
        .unwrap_or(viewport.width)
        .min(viewport.width);

    // Simple text layout - treat as a single paragraph
    let height = compute_text_height(text, styles, width);

    layout.push(LayoutBox {
        x: 0,
        y,
        width,
        height,
        kind: LayoutKind::Paragraph {
            text: text.to_string(),
        },
    });

    y += height + 12;
    layout.total_height = y;

    layout
}

/// Compute text height based on character count
fn compute_text_height(text: &str, styles: &StyleConfig, width: u32) -> u32 {
    let chars_per_line = (width / (styles.font_size / 2)).max(1);
    let lines = (text.len() as u32 / chars_per_line).max(1);
    (lines as f32 * styles.font_size as f32 * styles.line_height) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::queries::source::SourceStorage;
    use std::sync::Arc;

    #[test]
    fn test_layout_simple_document() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let doc_id = DocId::new("test");

        storage.set_document(doc_id.clone(), "# Test\n\nThis is a test.".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let viewport = Viewport::new(800, 600);
        let layout = db.query::<LayoutDocumentQuery>((doc_id, viewport));

        assert!(layout.total_height > 0);
    }

    #[test]
    fn test_viewport() {
        let viewport = Viewport::new(1024, 768);
        assert_eq!(viewport.width, 1024);
        assert_eq!(viewport.height, 768);
    }

    #[test]
    fn test_style_config() {
        let styles = StyleConfig::default();
        assert_eq!(styles.font_size, 16);
        assert_eq!(styles.line_height, 1.5);
    }

    #[test]
    fn test_layout_block_simple() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(1);

        storage.set_block(block_id.clone(), "This is block content.".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let viewport = Viewport::new(800, 600);
        let layout = db.query::<LayoutBlockQuery>((block_id, viewport));

        assert!(layout.total_height > 0);
    }

    #[test]
    fn test_layout_block_empty() {
        let db = Db::new();
        let storage = Arc::new(SourceStorage::new());
        let block_id = BlockId(1);

        storage.set_block(block_id.clone(), "".to_string());
        db.set_any("source_storage".to_string(), Box::new(storage));

        let viewport = Viewport::new(800, 600);
        let layout = db.query::<LayoutBlockQuery>((block_id, viewport));

        assert!(layout.boxes.is_empty());
        assert_eq!(layout.total_height, 0);
    }
}
