//! Integration tests for the document processing pipeline
//!
//! These tests verify that all the crates work together correctly.

use monowiki_core::DocumentPipeline;
use monowiki_types::{BlockId, DocChange, DocId};

#[test]
fn test_pipeline_creation() {
    let pipeline = DocumentPipeline::new();
    assert!(std::sync::Arc::strong_count(pipeline.db()) >= 1);
}

#[test]
fn test_invalidation_increases_revision() {
    let doc_id = DocId::new("test-doc");
    let pipeline = DocumentPipeline::new();

    let rev1 = pipeline.db().revision();

    // Trigger a change
    let change = DocChange::TextChanged {
        block_id: BlockId::new(1),
        start: 0,
        end: 5,
        new_text: "HELLO".to_string(),
    };

    pipeline.on_change(&doc_id, change);

    let rev2 = pipeline.db().revision();

    // Revision should increase
    assert!(rev2.0 > rev1.0);
}

#[test]
fn test_batch_invalidation() {
    let doc_id = DocId::new("test-doc");
    let pipeline = DocumentPipeline::new();

    let changes = vec![
        DocChange::TextChanged {
            block_id: BlockId::new(1),
            start: 0,
            end: 5,
            new_text: "hello".to_string(),
        },
        DocChange::BlockInserted {
            block_id: BlockId::new(2),
            parent_id: BlockId::new(0),
            position: 1,
        },
        DocChange::MarkChanged {
            block_id: BlockId::new(1),
            mark_type: "bold".to_string(),
            start: 0,
            end: 5,
        },
    ];

    // Should not panic
    pipeline.on_changes(&doc_id, changes);
}

#[test]
fn test_content_kinds() {
    // Test that content kinds are properly shared across crates
    use monowiki_types::ContentKind;

    assert!(ContentKind::Block.is_subkind_of(&ContentKind::Content));
    assert!(ContentKind::Inline.is_subkind_of(&ContentKind::Content));
    assert!(!ContentKind::Content.is_subkind_of(&ContentKind::Block));
}
