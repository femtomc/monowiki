//! Integration tests for the document processing pipeline
//!
//! These tests verify that all the crates work together correctly.

use monowiki_core::{DocumentPipeline, PipelineError};
use monowiki_types::{BlockId, DocChange, DocId};

#[test]
fn test_simple_document() {
    let source = r#"
# Hello World

This is a *test* document.
"#;

    let pipeline = DocumentPipeline::new();
    let content = pipeline.process_source(source).unwrap();

    // Verify we got a sequence of content
    assert!(matches!(content, monowiki_mrl::Content::Sequence(_)));
}

#[test]
fn test_mrl_text_function() {
    let source = r#"!text("Hello, MRL!")"#;

    let pipeline = DocumentPipeline::new();
    let result = pipeline.process_source(source);

    // Should parse and expand successfully
    assert!(result.is_ok());
}

#[test]
fn test_cached_pipeline() {
    let source = "# Test Document\n\nSome content here.";
    let doc_id = DocId::new("test-doc");

    let pipeline = DocumentPipeline::new();

    // First query - will compute
    let rev1 = pipeline.db().revision();
    let content1 = pipeline.process_cached(&doc_id, source).unwrap();

    // Second query - should use cache
    let rev2 = pipeline.db().revision();
    let content2 = pipeline.process_cached(&doc_id, source).unwrap();

    // Revision should not change (cached)
    assert_eq!(rev1, rev2);

    // Results should be the same
    assert_eq!(format!("{:?}", content1), format!("{:?}", content2));
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
fn test_execute_with_interpreter() {
    let source = "!text(\"Generated content\")";

    let pipeline = DocumentPipeline::new();
    let result = pipeline.execute(source);

    // Should execute successfully
    assert!(result.is_ok());
}

#[test]
fn test_mixed_prose_and_code() {
    let source = r#"
# Document with MRL

Regular prose here.

!text("Dynamic content")

More prose.
"#;

    let pipeline = DocumentPipeline::new();
    let content = pipeline.process_source(source);

    assert!(content.is_ok());
}

#[test]
fn test_error_handling() {
    // Invalid MRL syntax
    let source = "!unclosed_paren(";

    let pipeline = DocumentPipeline::new();
    let result = pipeline.process_source(source);

    // Should return an error
    assert!(result.is_err());
}

#[test]
fn test_type_checking_integration() {
    // This should type-check correctly
    let source = "!text(\"valid\")";

    let pipeline = DocumentPipeline::new();
    let result = pipeline.process_source(source);

    assert!(result.is_ok());
}

#[test]
fn test_content_kinds() {
    // Test that content kinds are properly shared across crates
    use monowiki_types::ContentKind;

    assert!(ContentKind::Block.is_subkind_of(&ContentKind::Content));
    assert!(ContentKind::Inline.is_subkind_of(&ContentKind::Content));
    assert!(!ContentKind::Content.is_subkind_of(&ContentKind::Block));
}
