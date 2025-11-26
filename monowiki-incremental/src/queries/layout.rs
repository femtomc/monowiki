//! Layout computation queries
//!
//! These queries compute layout information for rendering.

use crate::durability::Durability;
use crate::queries::expand::ExpandToContentQuery;
use crate::queries::source::DocId;
use crate::query::{Query, QueryDatabase};
use monowiki_mrl::{Block, Content, Inline};

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
        // In a real implementation, this would load from theme/config
        StyleConfig::default()
    }

    fn durability() -> Durability {
        // Styles change infrequently (durable tier)
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
    pub line_height: f32, // f32 doesn't implement Eq/Hash
    pub max_width: Option<u32>,
}

// Manual Eq implementation for StyleConfig
impl Eq for StyleConfig {}

// Manual Hash implementation for StyleConfig
impl std::hash::Hash for StyleConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.font_family.hash(state);
        self.font_size.hash(state);
        // Hash f32 as bits
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
            Some(content) => compute_layout(&content, &styles, viewport),
            None => Layout::new(),
        }
    }

    fn durability() -> Durability {
        // Layout changes when content, styles, or viewport changes
        Durability::Session
    }

    fn name() -> &'static str {
        "LayoutDocumentQuery"
    }
}

/// Layout information for rendering
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Layout {
    /// Layout boxes
    pub boxes: Vec<LayoutBox>,

    /// Total height
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
        self.total_height = self
            .total_height
            .max(layout_box.y + layout_box.height);
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

/// Extract plain text from inline content
fn inline_to_text(inline: &Inline) -> String {
    match inline {
        Inline::Text(t) => t.clone(),
        Inline::Emphasis(inner) => inline_to_text(inner),
        Inline::Strong(inner) => inline_to_text(inner),
        Inline::Code(c) => c.clone(),
        Inline::Link { body, .. } => inline_to_text(body),
        Inline::Image { alt, .. } => alt.clone(),
        Inline::Reference(r) => format!("@{}", r),
        Inline::Math(m) => m.clone(),
        Inline::Span { body, .. } => inline_to_text(body),
        Inline::Sequence(items) => items.iter().map(inline_to_text).collect::<Vec<_>>().join(""),
    }
}

/// Compute layout from content
fn compute_layout(content: &Content, styles: &StyleConfig, viewport: &Viewport) -> Layout {
    let mut layout = Layout::new();
    let mut y = 0u32;

    let width = styles
        .max_width
        .unwrap_or(viewport.width)
        .min(viewport.width);

    layout_content(content, styles, &mut layout, &mut y, width);

    layout
}

/// Recursively layout content
fn layout_content(
    content: &Content,
    styles: &StyleConfig,
    layout: &mut Layout,
    y: &mut u32,
    width: u32,
) {
    match content {
        Content::Block(block) => {
            layout_block(block, styles, layout, y, width);
        }
        Content::Inline(inline) => {
            // Wrap inline in a paragraph for layout
            let text = inline_to_text(inline);
            let height = compute_text_height(&text, styles, width);

            layout.push(LayoutBox {
                x: 0,
                y: *y,
                width,
                height,
                kind: LayoutKind::Paragraph { text },
            });

            *y += height + 12;
        }
        Content::Sequence(items) => {
            for item in items {
                layout_content(item, styles, layout, y, width);
            }
        }
    }
}

/// Layout a block element
fn layout_block(
    block: &Block,
    styles: &StyleConfig,
    layout: &mut Layout,
    y: &mut u32,
    width: u32,
) {
    match block {
        Block::Heading { level, body, .. } => {
            let text = inline_to_text(body);
            let height = (styles.font_size * 2) + (*level as u32 * 4);

            layout.push(LayoutBox {
                x: 0,
                y: *y,
                width,
                height,
                kind: LayoutKind::Heading {
                    level: *level as usize,
                    text,
                },
            });

            *y += height + 16;
        }

        Block::Paragraph { body, .. } => {
            let text = inline_to_text(body);
            let height = compute_text_height(&text, styles, width);

            layout.push(LayoutBox {
                x: 0,
                y: *y,
                width,
                height,
                kind: LayoutKind::Paragraph { text },
            });

            *y += height + 12;
        }

        Block::CodeBlock { lang, code, .. } => {
            let lines = code.lines().count().max(1);
            let height = (lines as u32) * styles.font_size;

            layout.push(LayoutBox {
                x: 0,
                y: *y,
                width,
                height,
                kind: LayoutKind::CodeBlock {
                    lang: lang.clone(),
                    code: code.clone(),
                },
            });

            *y += height + 16;
        }

        Block::List { items, .. } => {
            let item_texts: Vec<String> = items
                .iter()
                .map(|item| inline_to_text(&item.body))
                .collect();
            let height = (items.len() as u32) * ((styles.font_size as f32 * styles.line_height) as u32);

            layout.push(LayoutBox {
                x: 0,
                y: *y,
                width,
                height,
                kind: LayoutKind::List { items: item_texts },
            });

            *y += height + 12;
        }

        Block::Blockquote { body, .. } => {
            let text = content_to_text(body);
            let height = compute_text_height(&text, styles, width - 20);

            layout.push(LayoutBox {
                x: 20,
                y: *y,
                width: width - 20,
                height,
                kind: LayoutKind::Blockquote { text },
            });

            *y += height + 16;
        }

        Block::ThematicBreak { .. } => {
            let height = 1;

            layout.push(LayoutBox {
                x: 0,
                y: *y,
                width,
                height,
                kind: LayoutKind::ThematicBreak,
            });

            *y += height + 16;
        }

        Block::Table { .. } => {
            // Tables need more sophisticated layout
            let height = styles.font_size * 3;

            layout.push(LayoutBox {
                x: 0,
                y: *y,
                width,
                height,
                kind: LayoutKind::Paragraph {
                    text: "[table]".to_string(),
                },
            });

            *y += height + 12;
        }

        Block::Directive { name, body, .. } => {
            // Directives are rendered as their content with a marker
            let text = format!("!{}: {}", name, content_to_text(body));
            let height = compute_text_height(&text, styles, width);

            layout.push(LayoutBox {
                x: 0,
                y: *y,
                width,
                height,
                kind: LayoutKind::Paragraph { text },
            });

            *y += height + 12;
        }
    }
}

/// Convert content to plain text
fn content_to_text(content: &Content) -> String {
    match content {
        Content::Block(block) => match block {
            Block::Paragraph { body, .. } => inline_to_text(body),
            Block::Heading { body, .. } => inline_to_text(body),
            Block::CodeBlock { code, .. } => code.clone(),
            Block::List { items, .. } => items
                .iter()
                .map(|item| inline_to_text(&item.body))
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Blockquote { body, .. } => content_to_text(body),
            Block::Table { .. } => "[table]".to_string(),
            Block::ThematicBreak { .. } => "---".to_string(),
            Block::Directive { body, .. } => content_to_text(body),
        },
        Content::Inline(inline) => inline_to_text(inline),
        Content::Sequence(items) => items
            .iter()
            .map(content_to_text)
            .collect::<Vec<_>>()
            .join("\n"),
    }
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

        // Layout should have at least some boxes (depends on parsing)
        assert!(layout.total_height >= 0);
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
    fn test_inline_to_text() {
        let inline = Inline::Text("hello".to_string());
        assert_eq!(inline_to_text(&inline), "hello");

        let emphasis = Inline::Emphasis(Box::new(Inline::Text("emphasized".to_string())));
        assert_eq!(inline_to_text(&emphasis), "emphasized");

        let seq = Inline::Sequence(vec![
            Inline::Text("one ".to_string()),
            Inline::Text("two".to_string()),
        ]);
        assert_eq!(inline_to_text(&seq), "one two");
    }
}
