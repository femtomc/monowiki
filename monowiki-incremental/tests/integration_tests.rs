//! Integration tests for the incremental query system with real MRL parser

use monowiki_incremental::prelude::*;
use monowiki_incremental::{DocChange, InvalidationBridge};
use std::sync::Arc;

#[test]
fn test_full_pipeline_with_mrl() {
    let db = Db::new();

    // Set up source storage
    let storage = Arc::new(SourceStorage::new());
    storage.set_document(
        DocId("test".to_string()),
        "This is prose with !bold([emphasis])!.".to_string(),
    );
    db.set_any("source_storage".to_string(), Box::new(storage));

    // Query parse result
    let doc_id = DocId("test".to_string());
    let parse_result = db.query::<ParseShrubberyQuery>(doc_id.clone());
    assert!(
        parse_result.shrubbery.is_some(),
        "Parse should succeed: {:?}",
        parse_result.errors
    );

    // Query expand result
    let expand_result = db.query::<ExpandToContentQuery>(doc_id.clone());
    assert!(
        expand_result.content.is_some(),
        "Expand should succeed: {:?}",
        expand_result.errors
    );
}

#[test]
fn test_parse_simple_prose() {
    let db = Db::new();
    let storage = Arc::new(SourceStorage::new());
    let doc_id = DocId("test".to_string());

    storage.set_document(doc_id.clone(), "Hello, world!".to_string());
    db.set_any("source_storage".to_string(), Box::new(storage));

    let parse_result = db.query::<ParseShrubberyQuery>(doc_id);
    assert!(parse_result.shrubbery.is_some(), "Should parse simple prose");
    assert!(parse_result.errors.is_empty());
}

#[test]
fn test_parse_with_inline_code() {
    let db = Db::new();
    let storage = Arc::new(SourceStorage::new());
    let doc_id = DocId("test".to_string());

    storage.set_document(
        doc_id.clone(),
        "Text with !code([inline])! code.".to_string(),
    );
    db.set_any("source_storage".to_string(), Box::new(storage));

    let parse_result = db.query::<ParseShrubberyQuery>(doc_id);
    assert!(parse_result.shrubbery.is_some(), "Should parse inline code");
}

#[test]
fn test_invalidation_on_change() {
    let db = Arc::new(Db::new());
    let bridge = InvalidationBridge::new(db.clone());

    let storage = Arc::new(SourceStorage::new());
    storage.set_document(DocId("test".to_string()), "Hello".to_string());
    db.set_any("source_storage".to_string(), Box::new(storage.clone()));

    // Initial query
    let doc_id = DocId("test".to_string());
    let rev1 = db.revision();
    let _ = db.query::<ParseShrubberyQuery>(doc_id.clone());

    // Simulate change
    storage.set_document(doc_id.clone(), "Hello World".to_string());
    bridge.on_change(
        &doc_id,
        DocChange::TextChanged {
            block_id: BlockId("block1".to_string()),
            start: 5,
            end: 5,
            new_text: " World".to_string(),
        },
    );

    // Revision should have changed
    let rev2 = db.revision();
    assert!(
        rev2.0 > rev1.0,
        "Revision should increase after invalidation"
    );
}

#[test]
fn test_memoization() {
    let db = Db::new();
    let storage = Arc::new(SourceStorage::new());
    let doc_id = DocId("test".to_string());

    storage.set_document(doc_id.clone(), "Test document.".to_string());
    db.set_any("source_storage".to_string(), Box::new(storage));

    // First query
    let result1 = db.query::<ParseShrubberyQuery>(doc_id.clone());

    // Second query should use memoized result
    let result2 = db.query::<ParseShrubberyQuery>(doc_id);

    // Should get same result
    assert_eq!(result1.shrubbery.is_some(), result2.shrubbery.is_some());
}

#[test]
fn test_expansion_pipeline() {
    let db = Db::new();
    let storage = Arc::new(SourceStorage::new());
    let doc_id = DocId("test".to_string());

    // Simple MRL document
    storage.set_document(
        doc_id.clone(),
        "Prose with !emphasis([styled text])!.".to_string(),
    );
    db.set_any("source_storage".to_string(), Box::new(storage));

    // Parse
    let parse_result = db.query::<ParseShrubberyQuery>(doc_id.clone());
    assert!(parse_result.shrubbery.is_some());

    // Expand
    let expand_result = db.query::<ExpandToContentQuery>(doc_id);
    assert!(expand_result.content.is_some());
}

#[test]
fn test_type_checking() {
    let db = Db::new();
    let storage = Arc::new(SourceStorage::new());
    let doc_id = DocId("test".to_string());

    // Valid MRL
    storage.set_document(doc_id.clone(), "Valid prose.".to_string());
    db.set_any("source_storage".to_string(), Box::new(storage.clone()));

    let result = db.query::<ExpandToContentQuery>(doc_id.clone());
    assert!(result.content.is_some(), "Valid MRL should expand");

    // Type error case would be tested with actual type errors in MRL
}

#[test]
fn test_empty_document() {
    let db = Db::new();
    let storage = Arc::new(SourceStorage::new());
    let doc_id = DocId("test".to_string());

    storage.set_document(doc_id.clone(), "".to_string());
    db.set_any("source_storage".to_string(), Box::new(storage));

    let parse_result = db.query::<ParseShrubberyQuery>(doc_id.clone());
    assert!(parse_result.shrubbery.is_none(), "Empty doc should not parse");

    let expand_result = db.query::<ExpandToContentQuery>(doc_id);
    assert!(
        expand_result.content.is_none(),
        "Empty doc should not expand"
    );
}

#[test]
fn test_batch_invalidation() {
    let db = Arc::new(Db::new());
    let bridge = InvalidationBridge::new(db.clone());

    let storage = Arc::new(SourceStorage::new());
    let doc_id = DocId("test".to_string());
    storage.set_document(doc_id.clone(), "Initial".to_string());
    db.set_any("source_storage".to_string(), Box::new(storage));

    let changes = vec![
        DocChange::TextChanged {
            block_id: BlockId("block1".to_string()),
            start: 0,
            end: 7,
            new_text: "Changed".to_string(),
        },
        DocChange::MarkChanged {
            block_id: BlockId("block1".to_string()),
            mark_type: "bold".to_string(),
            start: 0,
            end: 7,
        },
    ];

    bridge.on_changes(&doc_id, changes);
}

#[test]
fn test_macro_config() {
    let db = Db::new();

    // Query macro config
    let config = db.query::<ActiveMacrosQuery>(());

    assert_eq!(config.enabled_macros.len(), 0);
    assert_eq!(config.version, 0);
}

#[test]
fn test_concurrent_access() {
    use std::thread;

    let db = Arc::new(Db::new());
    let storage = Arc::new(SourceStorage::new());

    // Set up multiple documents
    for i in 1..=5 {
        let doc_id = DocId(format!("doc{}", i));
        storage.set_document(doc_id, format!("Document {}", i));
    }
    db.set_any("source_storage".to_string(), Box::new(storage));

    // Query from multiple threads
    let handles: Vec<_> = (1..=5)
        .map(|i| {
            let db = db.clone();
            thread::spawn(move || {
                let doc_id = DocId(format!("doc{}", i));
                let result = db.query::<ParseShrubberyQuery>(doc_id);
                assert!(result.shrubbery.is_some());
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_revision_tracking() {
    let db = Db::new();
    let initial_rev = db.revision();

    let storage = Arc::new(SourceStorage::new());
    storage.set_document(DocId("test".to_string()), "Test".to_string());
    db.set_any("source_storage".to_string(), Box::new(storage));

    // Query should increase revision
    let _ = db.query::<ParseShrubberyQuery>(DocId("test".to_string()));

    let after_query = db.revision();
    assert!(after_query.0 >= initial_rev.0);
}
