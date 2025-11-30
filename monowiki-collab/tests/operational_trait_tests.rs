//! Integration tests for the Loro-based collaborative document API.
//! These exercise the public surface (struct + sync hooks) rather than the
//! internal helpers tested inside the crate.

use monowiki_collab::crdt::{BlockKind, BlockMeta, LoroNoteDoc};
use serde_json::Value;

#[test]
fn loro_doc_starts_empty_and_clean() {
    let doc = LoroNoteDoc::new();
    assert!(doc.get_block_ids().is_empty());
    assert!(!doc.is_dirty());
    assert_eq!(doc.to_markdown(), "");
}

#[test]
fn loro_doc_parses_markdown_into_blocks() {
    let doc = LoroNoteDoc::new_with_content(
        Value::Object(Default::default()),
        "# Title\n\nParagraph text.",
    )
    .unwrap();

    let block_ids = doc.get_block_ids();
    assert_eq!(block_ids.len(), 2);

    let heading = doc.get_block_meta(&block_ids[0]).unwrap();
    assert_eq!(heading.kind, BlockKind::Heading);
    assert_eq!(doc.get_block_text(&block_ids[0]), "Title");

    let para = doc.get_block_meta(&block_ids[1]).unwrap();
    assert_eq!(para.kind, BlockKind::Paragraph);
    assert_eq!(doc.get_block_text(&block_ids[1]), "Paragraph text.");
}

#[test]
fn loro_snapshot_roundtrip_preserves_content() {
    let original = LoroNoteDoc::new_with_content(
        Value::Object(Default::default()),
        "# Hello\n\n- one\n- two\n",
    )
    .unwrap();

    let snapshot = original.export_snapshot().unwrap();

    let restored = LoroNoteDoc::new();
    restored.import_snapshot(&snapshot).unwrap();

    let restored_md = restored.to_markdown();
    assert!(restored_md.contains("# Hello"));
    assert!(restored_md.contains("- one"));
    assert!(restored_md.contains("- two"));
}

#[test]
fn loro_broadcasts_updates_on_changes() {
    let doc = LoroNoteDoc::new();
    let mut rx = doc.subscribe();

    // Insert a paragraph block; should broadcast a sync packet.
    let block_id = doc
        .insert_block(0, BlockMeta::new(BlockKind::Paragraph))
        .unwrap();
    doc.set_block_text(&block_id, "live").unwrap();

    let packet = rx.try_recv().expect("expected an update after mutation");
    assert!(!packet.payload.is_empty());
}
