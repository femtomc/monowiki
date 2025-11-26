//! Migration utilities for converting from Yrs to Loro.
//!
//! This module provides tools for migrating existing documents from the Yrs
//! CRDT implementation to the Loro implementation, preserving content and
//! structure as much as possible.

use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::operational::{BlockKind, FractionalIndex, OperationalDoc};
use crate::yrs_adapter::YrsOperationalDoc;

#[cfg(feature = "loro")]
use crate::loro::LoroOperationalDoc;

/// Migration statistics and metadata.
#[derive(Debug, Clone)]
pub struct MigrationResult {
    pub blocks_migrated: usize,
    pub text_length: usize,
    pub marks_migrated: usize,
    pub warnings: Vec<String>,
}

/// Parse a Yrs document and extract structured blocks.
///
/// This analyzes the flat Y.Text content and produces a block structure
/// that can be imported into Loro's MovableTree.
pub fn parse_yrs_blocks(yrs_doc: &YrsOperationalDoc) -> Result<Vec<ParsedBlock>> {
    let tree = yrs_doc.get_tree()?;
    let mut blocks = Vec::new();

    for node in tree.nodes.values() {
        if node.id == tree.root {
            continue; // Skip root node
        }

        let text = yrs_doc.get_block_text(node.id.clone())?;
        let marks = yrs_doc.get_block_marks(node.id.clone())?;

        blocks.push(ParsedBlock {
            kind: node.kind.clone(),
            text,
            marks: marks
                .into_iter()
                .map(|m| ParsedMark {
                    mark_type: m.mark_type,
                    start: m.start.parse().unwrap_or(0),
                    end: m.end.parse().unwrap_or(0),
                    attrs: m.attrs,
                })
                .collect(),
            children: Vec::new(),
        });
    }

    Ok(blocks)
}

/// A block parsed from the source document.
#[derive(Debug, Clone)]
pub struct ParsedBlock {
    pub kind: BlockKind,
    pub text: String,
    pub marks: Vec<ParsedMark>,
    pub children: Vec<ParsedBlock>,
}

/// A formatting mark parsed from the source document.
#[derive(Debug, Clone)]
pub struct ParsedMark {
    pub mark_type: String,
    pub start: usize,
    pub end: usize,
    pub attrs: HashMap<String, String>,
}

/// Migrate from a Yrs document to a Loro document.
///
/// This performs the following steps:
/// 1. Parse the Yrs flat text into blocks
/// 2. Create corresponding MovableTree structure in Loro
/// 3. Populate text content for each block
/// 4. Apply formatting marks using Peritext
///
/// Returns statistics about the migration.
#[cfg(feature = "loro")]
pub fn migrate_yrs_to_loro(
    yrs_doc: &YrsOperationalDoc,
    loro_doc: &mut LoroOperationalDoc,
) -> Result<MigrationResult> {
    let mut result = MigrationResult {
        blocks_migrated: 0,
        text_length: 0,
        marks_migrated: 0,
        warnings: Vec::new(),
    };

    // Parse blocks from Yrs
    let blocks = parse_yrs_blocks(yrs_doc)?;

    // Get the root node from Loro's tree
    let loro_tree = loro_doc.get_tree()?;
    let root_id = loro_tree.root;

    // Import blocks into Loro
    for (idx, block) in blocks.iter().enumerate() {
        match import_block(loro_doc, &root_id, block, idx, &mut result) {
            Ok(_) => {
                result.blocks_migrated += 1;
                result.text_length += block.text.len();
            }
            Err(e) => {
                result.warnings.push(format!("Failed to import block {}: {}", idx, e));
            }
        }
    }

    Ok(result)
}

/// Import a single block into the Loro document.
#[cfg(feature = "loro")]
fn import_block(
    loro_doc: &mut LoroOperationalDoc,
    parent_id: &str,
    block: &ParsedBlock,
    position: usize,
    result: &mut MigrationResult,
) -> Result<String> {
    // Create the block in Loro's tree
    let block_id = loro_doc.insert_block(
        parent_id.to_string(),
        FractionalIndex::new(position.to_string()),
        block.kind.clone(),
    )?;

    // Insert the text content
    if !block.text.is_empty() {
        loro_doc.insert_text(block_id.clone(), 0, &block.text)?;
    }

    // Apply formatting marks
    for mark in &block.marks {
        match loro_doc.add_mark(
            block_id.clone(),
            &mark.mark_type,
            mark.start as u32,
            mark.end as u32,
            mark.attrs.clone(),
        ) {
            Ok(_) => result.marks_migrated += 1,
            Err(e) => {
                result.warnings.push(format!(
                    "Failed to apply mark '{}' at {}..{}: {}",
                    mark.mark_type, mark.start, mark.end, e
                ));
            }
        }
    }

    // Recursively import children
    for (idx, child) in block.children.iter().enumerate() {
        import_block(loro_doc, &block_id, child, idx, result)?;
    }

    Ok(block_id)
}

/// Export a Yrs document to a migration-friendly format.
///
/// This creates a JSON representation that can be:
/// - Stored as a migration checkpoint
/// - Used to verify migration correctness
/// - Imported into different systems
pub fn export_yrs_for_migration(yrs_doc: &YrsOperationalDoc) -> Result<String> {
    let blocks = parse_yrs_blocks(yrs_doc)?;

    let export = MigrationExport {
        version: "1.0".to_string(),
        source: "yrs".to_string(),
        blocks,
    };

    serde_json::to_string_pretty(&export).map_err(|e| anyhow!("Serialization failed: {}", e))
}

/// Migration export format.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MigrationExport {
    pub version: String,
    pub source: String,
    pub blocks: Vec<ParsedBlock>,
}

// Implement Serialize/Deserialize for ParsedBlock
impl serde::Serialize for ParsedBlock {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ParsedBlock", 4)?;
        state.serialize_field("kind", &format!("{:?}", self.kind))?;
        state.serialize_field("text", &self.text)?;
        state.serialize_field("marks", &self.marks)?;
        state.serialize_field("children", &self.children)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for ParsedBlock {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};

        struct ParsedBlockVisitor;

        impl<'de> Visitor<'de> for ParsedBlockVisitor {
            type Value = ParsedBlock;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct ParsedBlock")
            }

            fn visit_map<V>(self, mut map: V) -> std::result::Result<ParsedBlock, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut text = None;
                let mut marks = None;
                let mut children = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "kind" => {
                            // Skip kind for now - we'll default to Paragraph
                            let _: String = map.next_value()?;
                        }
                        "text" => text = Some(map.next_value()?),
                        "marks" => marks = Some(map.next_value()?),
                        "children" => children = Some(map.next_value()?),
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                Ok(ParsedBlock {
                    kind: BlockKind::Paragraph, // Default
                    text: text.ok_or_else(|| de::Error::missing_field("text"))?,
                    marks: marks.unwrap_or_default(),
                    children: children.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_struct(
            "ParsedBlock",
            &["kind", "text", "marks", "children"],
            ParsedBlockVisitor,
        )
    }
}

impl serde::Serialize for ParsedMark {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ParsedMark", 4)?;
        state.serialize_field("mark_type", &self.mark_type)?;
        state.serialize_field("start", &self.start)?;
        state.serialize_field("end", &self.end)?;
        state.serialize_field("attrs", &self.attrs)?;
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for ParsedMark {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};

        struct ParsedMarkVisitor;

        impl<'de> Visitor<'de> for ParsedMarkVisitor {
            type Value = ParsedMark;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct ParsedMark")
            }

            fn visit_map<V>(self, mut map: V) -> std::result::Result<ParsedMark, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut mark_type = None;
                let mut start = None;
                let mut end = None;
                let mut attrs = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "mark_type" => mark_type = Some(map.next_value()?),
                        "start" => start = Some(map.next_value()?),
                        "end" => end = Some(map.next_value()?),
                        "attrs" => attrs = Some(map.next_value()?),
                        _ => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }

                Ok(ParsedMark {
                    mark_type: mark_type.ok_or_else(|| de::Error::missing_field("mark_type"))?,
                    start: start.ok_or_else(|| de::Error::missing_field("start"))?,
                    end: end.ok_or_else(|| de::Error::missing_field("end"))?,
                    attrs: attrs.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_struct(
            "ParsedMark",
            &["mark_type", "start", "end", "attrs"],
            ParsedMarkVisitor,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yrs_blocks() {
        let yrs_doc = YrsOperationalDoc::new();

        // Insert some test text
        use yrs::{Transact, WriteTxn, Text};
        {
            let mut txn = yrs_doc.doc().transact_mut();
            let text = txn.get_or_insert_text("body");
            text.insert(&mut txn, 0, "# Heading\n\nParagraph text.");
        }

        let blocks = parse_yrs_blocks(&yrs_doc).unwrap();
        assert!(blocks.len() >= 1);
    }

    #[test]
    fn test_export_yrs_for_migration() {
        let yrs_doc = YrsOperationalDoc::new();

        use yrs::{Transact, WriteTxn, Text};
        {
            let mut txn = yrs_doc.doc().transact_mut();
            let text = txn.get_or_insert_text("body");
            text.insert(&mut txn, 0, "Test content");
        }

        let json = export_yrs_for_migration(&yrs_doc).unwrap();
        assert!(json.contains("Test content"));
    }

    #[cfg(feature = "loro")]
    #[test]
    fn test_migrate_yrs_to_loro() {
        let yrs_doc = YrsOperationalDoc::new();

        use yrs::{Transact, WriteTxn, Text};
        {
            let mut txn = yrs_doc.doc().transact_mut();
            let text = txn.get_or_insert_text("body");
            text.insert(&mut txn, 0, "# Heading\n\nParagraph");
        }

        let mut loro_doc = LoroOperationalDoc::new();
        let result = migrate_yrs_to_loro(&yrs_doc, &mut loro_doc).unwrap();

        assert!(result.blocks_migrated > 0);
        assert!(result.text_length > 0);
    }
}
