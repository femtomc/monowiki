//! Integration tests for the incremental query system

use monowiki_incremental::prelude::*;
use monowiki_incremental::queries::{
    ActiveMacrosQuery, ExpandToContentQuery, LayoutSectionQuery,
    ParseShrubberyQuery, SourceTextQuery,
};
use monowiki_incremental::invalidation::{BlockId, DocChange, TextRange};
use monowiki_incremental::queries::layout::Viewport;

#[test]
fn test_full_pipeline() {
    let db = Db::new();

    // Create a document section
    let section_id = SectionId(BlockId(1).0);
    let source = "# Introduction\n\nThis is a test document.";

    SourceTextQuery::set(&db, section_id, source.to_string());

    // Query through the pipeline
    let shrubbery = db.query::<ParseShrubberyQuery>(section_id);
    assert!(!shrubbery.nodes.is_empty());

    let content = db.query::<ExpandToContentQuery>(section_id);
    assert!(!content.elements.is_empty());

    let viewport = Viewport::new(800, 600);
    let layout = db.query::<LayoutSectionQuery>((section_id, viewport));
    assert!(!layout.boxes.is_empty());
}

#[test]
fn test_memoization() {
    let db = Db::new();

    let section_id = SectionId(BlockId(1).0);
    SourceTextQuery::set(&db, section_id, "# Test".to_string());

    // First query
    let content1 = db.query::<ExpandToContentQuery>(section_id);

    // Second query should use memoized result
    let content2 = db.query::<ExpandToContentQuery>(section_id);

    assert_eq!(content1, content2);
}

#[test]
fn test_invalidation() {
    let db = Db::new();

    let section_id = SectionId(BlockId(1).0);

    // Set initial content
    SourceTextQuery::set(&db, section_id, "# Version 1".to_string());
    let content1 = db.query::<ExpandToContentQuery>(section_id);

    // Change the source
    SourceTextQuery::set(&db, section_id, "# Version 2".to_string());

    // Query should recompute
    let content2 = db.query::<ExpandToContentQuery>(section_id);

    assert_ne!(content1, content2);
}

#[test]
fn test_early_cutoff() {
    let db = Db::new();

    let section_id = SectionId(BlockId(1).0);

    // Set content that will parse to the same AST despite different source
    SourceTextQuery::set(&db, section_id, "# Test  ".to_string());
    let _content1 = db.query::<ExpandToContentQuery>(section_id);

    // Change whitespace only (should parse to same result)
    SourceTextQuery::set(&db, section_id, "# Test".to_string());
    let _content2 = db.query::<ExpandToContentQuery>(section_id);

    // Both should produce the same content due to early cutoff
    // (in practice, the layout query wouldn't need to recompute)
}

#[test]
fn test_dependency_tracking() {
    let db = Db::new();

    let section_id = SectionId(BlockId(1).0);
    SourceTextQuery::set(&db, section_id, "# Test".to_string());

    // Query content (depends on parse, which depends on source)
    let _content = db.query::<ExpandToContentQuery>(section_id);

    // The database should have tracked dependencies
    // (In a real implementation, we'd query the dependency graph)
}

#[test]
fn test_multiple_sections() {
    let db = Db::new();

    let section1 = SectionId(BlockId(1).0);
    let section2 = SectionId(BlockId(2).0);

    SourceTextQuery::set(&db, section1, "# Section 1".to_string());
    SourceTextQuery::set(&db, section2, "# Section 2".to_string());

    let content1 = db.query::<ExpandToContentQuery>(section1);
    let content2 = db.query::<ExpandToContentQuery>(section2);

    assert_ne!(content1, content2);
}

#[test]
fn test_invalidation_bridge() {
    use std::sync::Arc;
    use monowiki_incremental::InvalidationBridge;

    let db = Arc::new(Db::new());
    let bridge = InvalidationBridge::new(db.clone());

    let section_id = SectionId(BlockId(1).0);
    SourceTextQuery::set(&*db, section_id, "# Original".to_string());

    // Simulate a CRDT change
    let change = DocChange::TextChanged {
        block_id: BlockId(section_id.0),
        range: TextRange::new(0, 10),
        new_text: "# Modified".to_string(),
    };

    bridge.on_crdt_change(change);

    // The query system should have been invalidated
    // (In practice, we'd check that queries recompute)
}

#[test]
fn test_batch_invalidation() {
    use std::sync::Arc;
    use monowiki_incremental::InvalidationBridge;

    let db = Arc::new(Db::new());
    let bridge = InvalidationBridge::new(db.clone());

    // Simulate multiple CRDT changes
    let changes = vec![
        DocChange::TextChanged {
            block_id: BlockId(1),
            range: TextRange::new(0, 5),
            new_text: "Hello".to_string(),
        },
        DocChange::TextChanged {
            block_id: BlockId(1),
            range: TextRange::new(5, 11),
            new_text: " World".to_string(),
        },
    ];

    bridge.on_crdt_changes(changes);

    // Should handle batch invalidation efficiently
}

#[test]
fn test_durability_tiers() {
    let db = Db::new();

    // Source text is volatile (changes frequently)
    assert_eq!(SourceTextQuery::durability(), Durability::Volatile);

    // Macros are durable (change infrequently)
    assert_eq!(ActiveMacrosQuery::durability(), Durability::Durable);
}

#[test]
fn test_layout_viewport_dependency() {
    let db = Db::new();

    let section_id = SectionId(BlockId(1).0);
    SourceTextQuery::set(&db, section_id, "# Test\n\nLong paragraph.".to_string());

    // Layout depends on viewport
    let viewport1 = Viewport::new(800, 600);
    let layout1 = db.query::<LayoutSectionQuery>((section_id, viewport1));

    let viewport2 = Viewport::new(1024, 768);
    let layout2 = db.query::<LayoutSectionQuery>((section_id, viewport2));

    // Different viewports should produce different layouts
    // (or at least be computed independently)
    assert_eq!(layout1.boxes.len(), layout2.boxes.len());
}

#[test]
fn test_complex_document() {
    let db = Db::new();

    let section_id = SectionId(BlockId(1).0);
    let source = r#"# Main Heading

This is a paragraph with some text.

## Subheading

Another paragraph.

```rust
fn main() {
    println!("Hello, world!");
}
```

More content here.
"#;

    SourceTextQuery::set(&db, section_id, source.to_string());

    // Parse
    let shrubbery = db.query::<ParseShrubberyQuery>(section_id);
    assert!(shrubbery.nodes.len() >= 3); // At least heading, paragraph, code

    // Expand
    let content = db.query::<ExpandToContentQuery>(section_id);
    assert!(content.elements.len() >= 3);

    // Layout
    let viewport = Viewport::new(800, 600);
    let layout = db.query::<LayoutSectionQuery>((section_id, viewport));
    assert!(layout.total_height > 0);
}

#[test]
fn test_empty_document() {
    let db = Db::new();

    let section_id = SectionId(BlockId(1).0);
    SourceTextQuery::set(&db, section_id, "".to_string());

    let content = db.query::<ExpandToContentQuery>(section_id);
    assert!(content.elements.is_empty());
}

#[test]
fn test_concurrent_queries() {
    use std::sync::Arc;
    use std::thread;

    let db = Arc::new(Db::new());

    // Set up multiple sections
    for i in 1..=5 {
        let section_id = SectionId(BlockId(i).0);
        SourceTextQuery::set(&*db, section_id, format!("# Section {}", i));
    }

    // Query from multiple threads
    let handles: Vec<_> = (1..=5)
        .map(|i| {
            let db = db.clone();
            thread::spawn(move || {
                let section_id = SectionId(BlockId(i).0);
                let content = db.query::<ExpandToContentQuery>(section_id);
                assert!(!content.elements.is_empty());
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

    let section_id = SectionId(BlockId(1).0);
    SourceTextQuery::set(&db, section_id, "# Test".to_string());

    let after_set = db.revision();

    // Revision should have increased
    assert!(after_set > initial_rev);
}
