//! Projection from operational CRDT state to semantic Content.
//!
//! This module provides utilities for converting the operational document
//! representation (CRDT-based tree + text + marks) into the semantic Content
//! type used by the rest of the system.
//!
//! The projection is deterministic and read-only: it derives semantic content
//! from the operational state without modifying it.

use anyhow::{anyhow, Result};
use std::collections::HashMap;

// Note: In a real implementation, we'd import these from monowiki-mrl
// For now, we'll use simplified local types that mirror the MRL types
use crate::operational::{
    BlockKind, DocTree, Mark, NodeId, OperationalDoc,
};

/// Simplified Content type (mirrors monowiki-mrl::content::Content)
#[derive(Debug, Clone)]
pub enum Content {
    Block(Block),
    Inline(Inline),
    Sequence(Vec<Content>),
}

/// Simplified Block type (mirrors monowiki-mrl::content::Block)
#[derive(Debug, Clone)]
pub enum Block {
    Heading {
        level: u8,
        body: Box<Inline>,
    },
    Paragraph {
        body: Box<Inline>,
    },
    CodeBlock {
        lang: Option<String>,
        code: String,
    },
    List {
        items: Vec<ListItem>,
        ordered: bool,
    },
    Blockquote {
        body: Box<Content>,
    },
}

/// Simplified ListItem type
#[derive(Debug, Clone)]
pub struct ListItem {
    pub body: Inline,
    pub nested: Option<Vec<ListItem>>,
}

/// Simplified Inline type (mirrors monowiki-mrl::content::Inline)
#[derive(Debug, Clone)]
pub enum Inline {
    Text(String),
    Emphasis(Box<Inline>),
    Strong(Box<Inline>),
    Code(String),
    Link {
        body: Box<Inline>,
        url: String,
        title: Option<String>,
    },
    Sequence(Vec<Inline>),
}

impl Inline {
    fn empty() -> Self {
        Inline::Text(String::new())
    }
}

/// Project the complete document from operational to semantic representation.
///
/// This is the main entry point for converting CRDT state into Content.
pub fn project_to_content(doc: &dyn OperationalDoc) -> Result<Content> {
    let tree = doc.get_tree()?;
    project_node(&tree, &tree.root, doc)
}

/// Project a single node and its children recursively.
fn project_node(
    tree: &DocTree,
    node_id: &NodeId,
    doc: &dyn OperationalDoc,
) -> Result<Content> {
    let node = tree
        .nodes
        .get(node_id)
        .ok_or_else(|| anyhow!("Node not found: {}", node_id))?;

    match &node.kind {
        BlockKind::Section => {
            // A section is just a container - project all children
            let children: Result<Vec<Content>> = node
                .children
                .iter()
                .map(|child_id| project_node(tree, child_id, doc))
                .collect();

            Ok(Content::Sequence(children?))
        }

        BlockKind::Heading { level } => {
            let text = doc.get_block_text(node.id.clone())?;
            let marks = doc.get_block_marks(node.id.clone())?;
            let inline = project_inline(&text, &marks)?;

            Ok(Content::Block(Block::Heading {
                level: *level,
                body: Box::new(inline),
            }))
        }

        BlockKind::Paragraph => {
            let text = doc.get_block_text(node.id.clone())?;
            let marks = doc.get_block_marks(node.id.clone())?;
            let inline = project_inline(&text, &marks)?;

            Ok(Content::Block(Block::Paragraph {
                body: Box::new(inline),
            }))
        }

        BlockKind::CodeBlock => {
            let text = doc.get_block_text(node.id.clone())?;

            // Parse language from first line if present
            let (lang, code) = if text.starts_with("```") {
                let mut lines = text.lines();
                let first = lines.next().unwrap_or("");
                let lang = first.trim_start_matches("```").trim();
                let rest: Vec<&str> = lines.collect();
                let code = rest.join("\n");

                (
                    if lang.is_empty() {
                        None
                    } else {
                        Some(lang.to_string())
                    },
                    code,
                )
            } else {
                (None, text)
            };

            Ok(Content::Block(Block::CodeBlock { lang, code }))
        }

        BlockKind::List => {
            // Project list items
            let mut items = Vec::new();

            for child_id in &node.children {
                let child_text = doc.get_block_text(child_id.clone())?;
                let child_marks = doc.get_block_marks(child_id.clone())?;
                let inline = project_inline(&child_text, &child_marks)?;

                items.push(ListItem {
                    body: inline,
                    nested: None, // TODO: handle nested lists
                });
            }

            Ok(Content::Block(Block::List {
                items,
                ordered: false, // TODO: detect ordered vs unordered
            }))
        }

        BlockKind::ListItem => {
            // List items are handled by their parent List node
            let text = doc.get_block_text(node.id.clone())?;
            let marks = doc.get_block_marks(node.id.clone())?;
            let inline = project_inline(&text, &marks)?;

            Ok(Content::Inline(inline))
        }

        BlockKind::Blockquote => {
            // Project children of blockquote
            let children: Result<Vec<Content>> = node
                .children
                .iter()
                .map(|child_id| project_node(tree, child_id, doc))
                .collect();

            Ok(Content::Block(Block::Blockquote {
                body: Box::new(Content::Sequence(children?)),
            }))
        }
    }
}

/// Project plain text + marks into rich Inline content.
///
/// This implements the Peritext mark semantics:
/// - Marks with Before/After anchors expand differently with edits
/// - Overlapping marks are represented as nested Inline structures
fn project_inline(text: &str, marks: &[Mark]) -> Result<Inline> {
    if marks.is_empty() {
        return Ok(Inline::Text(text.to_string()));
    }

    // Build a map of character positions to active marks
    // This is a simplified implementation - a real one would handle:
    // - Anchor semantics (Before/After)
    // - Proper mark ordering and nesting
    // - Mark priority for overlaps

    let mut segments = Vec::new();
    let mut current_pos = 0;

    // Sort marks by start position
    let mut sorted_marks = marks.to_vec();
    sorted_marks.sort_by(|a, b| a.start.cmp(&b.start));

    for mark in &sorted_marks {
        // For now, we'll use a simplified approach where we convert CharId to position
        // In a real implementation, CharId would map to actual character positions
        let start_pos = parse_char_id(&mark.start).unwrap_or(0);
        let end_pos = parse_char_id(&mark.end).unwrap_or(text.len());

        // Add text before this mark
        if start_pos > current_pos {
            let segment = text
                .get(current_pos..start_pos)
                .unwrap_or("")
                .to_string();
            if !segment.is_empty() {
                segments.push(Inline::Text(segment));
            }
        }

        // Add marked text
        let marked_text = text
            .get(start_pos..end_pos)
            .unwrap_or("")
            .to_string();

        if !marked_text.is_empty() {
            let content = Box::new(Inline::Text(marked_text));

            let marked_inline = match mark.mark_type.as_str() {
                "emphasis" | "em" | "italic" => Inline::Emphasis(content),
                "strong" | "bold" => Inline::Strong(content),
                "code" => {
                    // Extract text from the box
                    if let Inline::Text(s) = *content {
                        Inline::Code(s)
                    } else {
                        Inline::Code(String::new())
                    }
                }
                "link" => {
                    let url = mark
                        .attrs
                        .get("href")
                        .or_else(|| mark.attrs.get("url"))
                        .cloned()
                        .unwrap_or_default();

                    let title = mark.attrs.get("title").cloned();

                    Inline::Link {
                        body: content,
                        url,
                        title,
                    }
                }
                _ => {
                    // Unknown mark type - just return the text
                    *content
                }
            };

            segments.push(marked_inline);
        }

        current_pos = end_pos;
    }

    // Add any remaining text
    if current_pos < text.len() {
        let segment = text.get(current_pos..).unwrap_or("").to_string();
        if !segment.is_empty() {
            segments.push(Inline::Text(segment));
        }
    }

    // Combine segments
    if segments.is_empty() {
        Ok(Inline::empty())
    } else if segments.len() == 1 {
        Ok(segments.into_iter().next().unwrap())
    } else {
        Ok(Inline::Sequence(segments))
    }
}

/// Parse a CharId into a character position.
///
/// This is a simplified implementation. In a real CRDT, CharIds are opaque
/// identifiers that maintain their meaning across concurrent edits.
fn parse_char_id(char_id: &str) -> Option<usize> {
    // Try to parse as a simple number
    char_id.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operational::{Anchor, FractionalIndex, TreeNode};

    #[test]
    fn test_project_inline_plain_text() {
        let text = "Hello, world!";
        let marks = vec![];

        let result = project_inline(text, &marks).unwrap();

        match result {
            Inline::Text(s) => assert_eq!(s, "Hello, world!"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_project_inline_with_emphasis() {
        let text = "Hello, world!";
        let marks = vec![Mark {
            id: "m1".to_string(),
            mark_type: "emphasis".to_string(),
            start: "0".to_string(),
            end: "5".to_string(),
            start_anchor: Anchor::Before,
            end_anchor: Anchor::After,
            attrs: HashMap::new(),
        }];

        let result = project_inline(text, &marks).unwrap();

        match result {
            Inline::Sequence(segments) => {
                assert_eq!(segments.len(), 2);
                // First segment should be emphasized "Hello"
                assert!(matches!(segments[0], Inline::Emphasis(_)));
                // Second segment should be ", world!"
                assert!(matches!(segments[1], Inline::Text(_)));
            }
            _ => panic!("Expected Sequence variant"),
        }
    }

    #[test]
    fn test_project_empty_tree() {
        use crate::operational::DocTree;

        let root = "root".to_string();
        let mut nodes = HashMap::new();

        nodes.insert(
            root.clone(),
            TreeNode {
                id: root.clone(),
                parent: None,
                kind: BlockKind::Section,
                position: FractionalIndex::first(),
                children: Vec::new(),
            },
        );

        let tree = DocTree { root, nodes };

        // We can't easily test project_node without a full OperationalDoc implementation
        // This is a placeholder test
        assert!(tree.nodes.len() == 1);
    }
}
