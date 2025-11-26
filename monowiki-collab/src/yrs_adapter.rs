//! Yrs adapter implementing OperationalDoc trait.
//!
//! This module wraps the existing Yrs-based CRDT implementation to conform
//! to the OperationalDoc trait, allowing it to be used interchangeably with
//! the Loro implementation.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use anyhow::{anyhow, Result};
use yrs::{Doc, Transact, ReadTxn, WriteTxn, GetString, Text};

use crate::operational::*;

/// Yrs-based implementation of OperationalDoc.
///
/// This is a compatibility adapter that wraps the existing Yrs implementation
/// (which uses a single Y.Text for the document body) to work with the new
/// OperationalDoc trait.
///
/// Note: This implementation has limitations compared to Loro:
/// - Single flat text instead of structured tree
/// - No native block-level operations
/// - Limited mark/formatting support
///
/// This adapter exists to maintain backward compatibility while we transition
/// to Loro.
pub struct YrsOperationalDoc {
    doc: Doc,
    subscriptions: Arc<Mutex<HashMap<SubscriptionId, Box<dyn Fn(DocChange) + Send>>>>,
    next_subscription_id: Arc<Mutex<SubscriptionId>>,
    /// Map of block IDs to text ranges (start, end)
    /// This is a simulation layer since Yrs doesn't have native blocks
    block_ranges: Arc<Mutex<HashMap<BlockId, (u32, u32)>>>,
}

impl YrsOperationalDoc {
    /// Create a new Yrs-based operational document.
    pub fn new() -> Self {
        let doc = Doc::new();
        Self {
            doc,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            next_subscription_id: Arc::new(Mutex::new(0)),
            block_ranges: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create from an existing Yrs Doc.
    pub fn from_doc(doc: Doc) -> Self {
        Self {
            doc,
            subscriptions: Arc::new(Mutex::new(HashMap::new())),
            next_subscription_id: Arc::new(Mutex::new(0)),
            block_ranges: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get the underlying Yrs document.
    pub fn doc(&self) -> &Doc {
        &self.doc
    }


    /// Parse the flat text into a simulated block structure.
    ///
    /// This is a heuristic approach: we split on blank lines and treat
    /// each chunk as a separate block.
    fn parse_blocks(&self) -> Vec<(BlockId, BlockKind, String)> {
        let text = {
            let mut txn = self.doc.transact_mut();
            let text_ref = txn.get_or_insert_text("body");
            text_ref.get_string(&txn)
        };

        let mut blocks = Vec::new();
        let mut current_text = String::new();
        let mut block_num = 0;

        for line in text.lines() {
            if line.trim().is_empty() {
                if !current_text.is_empty() {
                    let kind = if current_text.starts_with('#') {
                        let level = current_text.chars().take_while(|c| *c == '#').count();
                        BlockKind::Heading { level: level as u8 }
                    } else {
                        BlockKind::Paragraph
                    };

                    blocks.push((
                        format!("block_{}", block_num),
                        kind,
                        current_text.clone(),
                    ));
                    block_num += 1;
                    current_text.clear();
                }
            } else {
                if !current_text.is_empty() {
                    current_text.push('\n');
                }
                current_text.push_str(line);
            }
        }

        // Don't forget the last block
        if !current_text.is_empty() {
            let kind = if current_text.starts_with('#') {
                let level = current_text.chars().take_while(|c| *c == '#').count();
                BlockKind::Heading { level: level as u8 }
            } else {
                BlockKind::Paragraph
            };

            blocks.push((
                format!("block_{}", block_num),
                kind,
                current_text,
            ));
        }

        blocks
    }
}

impl Default for YrsOperationalDoc {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationalDoc for YrsOperationalDoc {
    fn get_tree(&self) -> Result<DocTree> {
        // Simulate a tree structure from the flat text
        let root = "root".to_string();
        let mut nodes = HashMap::new();

        // Root node
        let mut root_node = TreeNode {
            id: root.clone(),
            parent: None,
            kind: BlockKind::Section,
            position: FractionalIndex::first(),
            children: Vec::new(),
        };

        // Parse blocks and add them as children
        let blocks = self.parse_blocks();
        for (idx, (block_id, kind, _text)) in blocks.iter().enumerate() {
            root_node.children.push(block_id.clone());

            nodes.insert(block_id.clone(), TreeNode {
                id: block_id.clone(),
                parent: Some(root.clone()),
                kind: kind.clone(),
                position: FractionalIndex::new(idx.to_string()),
                children: Vec::new(),
            });
        }

        nodes.insert(root.clone(), root_node);

        Ok(DocTree { root, nodes })
    }

    fn get_block_text(&self, block_id: BlockId) -> Result<String> {
        // Find the block in our parsed structure
        let blocks = self.parse_blocks();

        blocks
            .iter()
            .find(|(id, _, _)| id == &block_id)
            .map(|(_, _, text)| text.clone())
            .ok_or_else(|| anyhow!("Block not found: {}", block_id))
    }

    fn get_block_marks(&self, _block_id: BlockId) -> Result<Vec<Mark>> {
        // Yrs doesn't have native mark support in our current implementation
        // We could parse markdown formatting, but for now return empty
        Ok(Vec::new())
    }

    fn insert_block(
        &mut self,
        _parent: NodeId,
        _position: FractionalIndex,
        _kind: BlockKind,
    ) -> Result<BlockId> {
        // Yrs doesn't have native block support
        // We'd need to insert text at the appropriate position
        // For now, return an error indicating this is not fully supported
        Err(anyhow!("Block insertion not fully supported in Yrs adapter"))
    }

    fn move_block(
        &mut self,
        _block_id: BlockId,
        _new_parent: NodeId,
        _new_position: FractionalIndex,
    ) -> Result<()> {
        Err(anyhow!("Block movement not supported in Yrs adapter"))
    }

    fn delete_block(&mut self, _block_id: BlockId) -> Result<()> {
        Err(anyhow!("Block deletion not fully supported in Yrs adapter"))
    }

    fn insert_text(&mut self, _block_id: BlockId, offset: u32, text: &str) -> Result<()> {
        // For simplicity, insert at the given offset in the global text
        // A real implementation would need to map block offsets to global offsets
        let mut txn = self.doc.transact_mut();
        let text_ref = txn.get_or_insert_text("body");
        text_ref.insert(&mut txn, offset, text);
        Ok(())
    }

    fn delete_text(&mut self, _block_id: BlockId, start: u32, end: u32) -> Result<()> {
        let len = end.saturating_sub(start);
        let mut txn = self.doc.transact_mut();
        let text_ref = txn.get_or_insert_text("body");
        text_ref.remove_range(&mut txn, start, len);
        Ok(())
    }

    fn add_mark(
        &mut self,
        _block_id: BlockId,
        _mark_type: &str,
        _start: u32,
        _end: u32,
        _attrs: MarkAttrs,
    ) -> Result<MarkId> {
        // Marks not natively supported in current Yrs implementation
        Err(anyhow!("Marks not supported in Yrs adapter"))
    }

    fn remove_mark(&mut self, _block_id: BlockId, _mark_id: MarkId) -> Result<()> {
        Err(anyhow!("Marks not supported in Yrs adapter"))
    }

    fn encode_state(&self) -> Result<Vec<u8>> {
        use yrs::{StateVector, updates::encoder::{Encode, Encoder, EncoderV1}};

        let txn = self.doc.transact();
        let state = txn.encode_state_as_update_v1(&StateVector::default());
        Ok(state)
    }

    fn apply_update(&mut self, update: &[u8]) -> Result<()> {
        use yrs::updates::decoder::{Decode, DecoderV1};
        use yrs::Update;

        let update = Update::decode_v1(update)
            .map_err(|e| anyhow!("Failed to decode update: {:?}", e))?;

        let mut txn = self.doc.transact_mut();
        txn.apply_update(update)?;

        Ok(())
    }

    fn subscribe(&self, callback: Box<dyn Fn(DocChange) + Send>) -> SubscriptionId {
        let mut subs = self.subscriptions.lock().unwrap();
        let mut next_id = self.next_subscription_id.lock().unwrap();

        let id = *next_id;
        *next_id += 1;

        subs.insert(id, callback);

        // In a real implementation, we'd subscribe to Yrs's observe events here
        // and trigger the callback when changes occur

        id
    }

    fn unsubscribe(&self, subscription_id: SubscriptionId) {
        let mut subs = self.subscriptions.lock().unwrap();
        subs.remove(&subscription_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_yrs_doc() {
        let doc = YrsOperationalDoc::new();
        assert!(doc.get_tree().is_ok());
    }

    #[test]
    fn test_parse_blocks() {
        let doc = YrsOperationalDoc::new();

        // Insert some text
        let text = "# Heading\n\nSome paragraph text.\n\nAnother paragraph.";
        {
            let mut txn = doc.doc.transact_mut();
            let text_ref = txn.get_or_insert_text("body");
            text_ref.insert(&mut txn, 0, text);
        }

        let blocks = doc.parse_blocks();
        assert_eq!(blocks.len(), 3);

        // Check first block is a heading
        assert!(matches!(blocks[0].1, BlockKind::Heading { level: 1 }));

        // Check other blocks are paragraphs
        assert!(matches!(blocks[1].1, BlockKind::Paragraph));
        assert!(matches!(blocks[2].1, BlockKind::Paragraph));
    }

    #[test]
    fn test_get_tree() {
        let doc = YrsOperationalDoc::new();

        // Insert some text
        {
            let mut txn = doc.doc.transact_mut();
            let text_ref = txn.get_or_insert_text("body");
            text_ref.insert(&mut txn, 0, "# Heading\n\nParagraph");
        }

        let tree = doc.get_tree().unwrap();
        assert_eq!(tree.nodes.len(), 3); // root + 2 blocks
    }

    #[test]
    fn test_encode_decode() {
        let mut doc1 = YrsOperationalDoc::new();

        // Insert text
        {
            let mut txn = doc1.doc.transact_mut();
            let text_ref = txn.get_or_insert_text("body");
            text_ref.insert(&mut txn, 0, "Hello, world!");
        }

        // Encode state
        let state = doc1.encode_state().unwrap();

        // Apply to new doc
        let mut doc2 = YrsOperationalDoc::new();
        doc2.apply_update(&state).unwrap();

        // Verify text matches
        let text1 = {
            let mut txn = doc1.doc.transact_mut();
            let text_ref = txn.get_or_insert_text("body");
            text_ref.get_string(&txn)
        };

        let text2 = {
            let mut txn = doc2.doc.transact_mut();
            let text_ref = txn.get_or_insert_text("body");
            text_ref.get_string(&txn)
        };

        assert_eq!(text1, text2);
    }
}
