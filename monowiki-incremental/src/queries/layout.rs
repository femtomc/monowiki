//! Layout computation queries
//!
//! These queries compute layout information for rendering.

use crate::durability::Durability;
use crate::invalidation::SectionId;
use crate::queries::expand::{Content, ExpandToContentQuery};
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
    pub line_height: f32,  // f32 doesn't implement Eq/Hash
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

/// Query for computing layout of a section
pub struct LayoutSectionQuery;

impl Query for LayoutSectionQuery {
    type Key = (SectionId, Viewport);
    type Value = Layout;

    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value {
        let (section_id, viewport) = key;

        // Get content (creates dependency)
        let content = db.query::<ExpandToContentQuery>(*section_id);

        // Get styles (creates dependency)
        let styles = db.query::<ActiveStylesQuery>(());

        // Compute layout
        compute_layout(&content, &styles, viewport)
    }

    fn durability() -> Durability {
        // Layout changes when content, styles, or viewport changes
        Durability::Session
    }

    fn name() -> &'static str {
        "LayoutSectionQuery"
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
}

/// Compute layout from content
fn compute_layout(content: &Content, styles: &StyleConfig, viewport: &Viewport) -> Layout {
    let mut layout = Layout::new();
    let mut y = 0;

    let width = styles
        .max_width
        .unwrap_or(viewport.width)
        .min(viewport.width);

    for element in &content.elements {
        use crate::queries::expand::{BlockElement, ContentElement};

        match element {
            ContentElement::Block(block) => match block {
                BlockElement::Heading { level, body } => {
                    let text = body
                        .iter()
                        .map(|inline| match inline {
                            crate::queries::expand::InlineElement::Text(t) => t.clone(),
                            _ => String::new(),
                        })
                        .collect::<Vec<_>>()
                        .join("");

                    let height = (styles.font_size * 2) + (*level as u32 * 4);

                    layout.push(LayoutBox {
                        x: 0,
                        y,
                        width,
                        height,
                        kind: LayoutKind::Heading {
                            level: *level,
                            text,
                        },
                    });

                    y += height + 16;
                }

                BlockElement::Paragraph { body } => {
                    let text = body
                        .iter()
                        .map(|inline| match inline {
                            crate::queries::expand::InlineElement::Text(t) => t.clone(),
                            _ => String::new(),
                        })
                        .collect::<Vec<_>>()
                        .join("");

                    // Simple height estimation based on character count
                    let chars_per_line = width / (styles.font_size / 2);
                    let lines = (text.len() as u32 / chars_per_line).max(1);
                    let height = lines * (styles.font_size as f32 * styles.line_height) as u32;

                    layout.push(LayoutBox {
                        x: 0,
                        y,
                        width,
                        height,
                        kind: LayoutKind::Paragraph { text },
                    });

                    y += height + 12;
                }

                BlockElement::CodeBlock { lang, code, .. } => {
                    // Code blocks use fixed-width font
                    let lines = code.lines().count().max(1);
                    let height = (lines as u32) * styles.font_size;

                    layout.push(LayoutBox {
                        x: 0,
                        y,
                        width,
                        height,
                        kind: LayoutKind::CodeBlock {
                            lang: lang.clone(),
                            code: code.clone(),
                        },
                    });

                    y += height + 16;
                }

                _ => {
                    // Handle other block types
                }
            },

            ContentElement::Inline(_) => {
                // Inline elements are handled within blocks
            }
        }
    }

    layout
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::invalidation::BlockId;
    use crate::queries::source::SourceTextQuery;

    #[test]
    fn test_layout_heading() {
        let db = Db::new();

        let section_id = SectionId(BlockId(1).0);
        SourceTextQuery::set(&db, section_id, "# Test".to_string());

        let viewport = Viewport::new(800, 600);
        let layout = db.query::<LayoutSectionQuery>((section_id, viewport));

        assert!(!layout.boxes.is_empty());
        match &layout.boxes[0].kind {
            LayoutKind::Heading { level, .. } => {
                assert_eq!(*level, 1);
            }
            _ => panic!("Expected heading layout"),
        }
    }

    #[test]
    fn test_layout_paragraph() {
        let db = Db::new();

        let section_id = SectionId(BlockId(1).0);
        SourceTextQuery::set(&db, section_id, "Test paragraph.".to_string());

        let viewport = Viewport::new(800, 600);
        let layout = db.query::<LayoutSectionQuery>((section_id, viewport));

        assert!(!layout.boxes.is_empty());
        match &layout.boxes[0].kind {
            LayoutKind::Paragraph { .. } => {}
            _ => panic!("Expected paragraph layout"),
        }
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
}
