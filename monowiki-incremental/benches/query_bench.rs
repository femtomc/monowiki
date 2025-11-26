//! Benchmarks for the incremental query system

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use monowiki_incremental::prelude::*;
use monowiki_incremental::queries::{
    ExpandToContentQuery, LayoutDocumentQuery, ParseShrubberyQuery, SourceStorage,
};
use monowiki_incremental::queries::layout::Viewport;
use std::sync::Arc;

/// Helper to set up a database with source storage
fn setup_db_with_source(doc_id: &DocId, source: &str) -> (Db, Arc<SourceStorage>) {
    let db = Db::new();
    let storage = Arc::new(SourceStorage::new());
    storage.set_document(doc_id.clone(), source.to_string());
    db.set_any("source_storage".to_string(), Box::new(storage.clone()));
    (db, storage)
}

fn bench_source_storage(c: &mut Criterion) {
    let doc_id = DocId::new("test");
    let storage = SourceStorage::new();

    c.bench_function("source_storage_set", |b| {
        b.iter(|| {
            storage.set_document(
                doc_id.clone(),
                black_box("# Test Heading\n\nParagraph.".to_string()),
            );
        })
    });

    storage.set_document(doc_id.clone(), "# Test".to_string());

    c.bench_function("source_storage_get", |b| {
        b.iter(|| {
            let text = storage.get_document(black_box(&doc_id));
            black_box(text);
        })
    });
}

fn bench_parse_query(c: &mut Criterion) {
    let doc_id = DocId::new("test");
    let source = "# Heading\n\nParagraph text.\n\n## Subheading\n\nMore text.";
    let (db, storage) = setup_db_with_source(&doc_id, source);

    c.bench_function("parse_query_cold", |b| {
        b.iter(|| {
            db.clear_all();
            db.set_any("source_storage".to_string(), Box::new(storage.clone()));
            let shrubbery = db.query::<ParseShrubberyQuery>(black_box(doc_id.clone()));
            black_box(shrubbery);
        })
    });

    c.bench_function("parse_query_hot", |b| {
        b.iter(|| {
            let shrubbery = db.query::<ParseShrubberyQuery>(black_box(doc_id.clone()));
            black_box(shrubbery);
        })
    });
}

fn bench_expand_query(c: &mut Criterion) {
    let doc_id = DocId::new("test");
    let source = "# Heading\n\nParagraph.\n\n```rust\nfn main() {}\n```";
    let (db, storage) = setup_db_with_source(&doc_id, source);

    c.bench_function("expand_query_cold", |b| {
        b.iter(|| {
            db.clear_all();
            db.set_any("source_storage".to_string(), Box::new(storage.clone()));
            let content = db.query::<ExpandToContentQuery>(black_box(doc_id.clone()));
            black_box(content);
        })
    });

    c.bench_function("expand_query_hot", |b| {
        b.iter(|| {
            let content = db.query::<ExpandToContentQuery>(black_box(doc_id.clone()));
            black_box(content);
        })
    });
}

fn bench_layout_query(c: &mut Criterion) {
    let doc_id = DocId::new("test");
    let viewport = Viewport::new(800, 600);
    let source = "# Title\n\nParagraph.\n\n## Section\n\nMore text.";
    let (db, storage) = setup_db_with_source(&doc_id, source);

    c.bench_function("layout_query_cold", |b| {
        b.iter(|| {
            db.clear_all();
            db.set_any("source_storage".to_string(), Box::new(storage.clone()));
            let layout =
                db.query::<LayoutDocumentQuery>(black_box((doc_id.clone(), viewport)));
            black_box(layout);
        })
    });

    c.bench_function("layout_query_hot", |b| {
        b.iter(|| {
            let layout =
                db.query::<LayoutDocumentQuery>(black_box((doc_id.clone(), viewport)));
            black_box(layout);
        })
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    let doc_id = DocId::new("test");
    let viewport = Viewport::new(800, 600);
    let source = "# Introduction\n\nThis is a test document with some content.";
    let (db, storage) = setup_db_with_source(&doc_id, source);

    c.bench_function("full_pipeline_cold", |b| {
        b.iter(|| {
            db.clear_all();
            db.set_any("source_storage".to_string(), Box::new(storage.clone()));

            let _shrubbery = db.query::<ParseShrubberyQuery>(doc_id.clone());
            let _content = db.query::<ExpandToContentQuery>(doc_id.clone());
            let layout = db.query::<LayoutDocumentQuery>((doc_id.clone(), viewport));
            black_box(layout);
        })
    });

    c.bench_function("full_pipeline_hot", |b| {
        b.iter(|| {
            let _shrubbery = db.query::<ParseShrubberyQuery>(black_box(doc_id.clone()));
            let _content = db.query::<ExpandToContentQuery>(black_box(doc_id.clone()));
            let layout =
                db.query::<LayoutDocumentQuery>(black_box((doc_id.clone(), viewport)));
            black_box(layout);
        })
    });
}

fn bench_invalidation(c: &mut Criterion) {
    let doc_id = DocId::new("test");
    let viewport = Viewport::new(800, 600);
    let storage = Arc::new(SourceStorage::new());
    let db = Db::new();
    db.set_any("source_storage".to_string(), Box::new(storage.clone()));

    c.bench_function("invalidate_and_recompute", |b| {
        let mut counter = 0;
        b.iter(|| {
            // Change source slightly
            let source = format!("# Version {}\n\nContent.", counter);
            storage.set_document(doc_id.clone(), source);
            db.invalidate::<ParseShrubberyQuery>(doc_id.clone());

            // Recompute pipeline
            let layout = db.query::<LayoutDocumentQuery>((doc_id.clone(), viewport));
            black_box(layout);

            counter += 1;
        })
    });
}

fn bench_multiple_documents(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_documents");

    for num_docs in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_docs),
            num_docs,
            |b, &num_docs| {
                let db = Db::new();
                let storage = Arc::new(SourceStorage::new());
                db.set_any("source_storage".to_string(), Box::new(storage.clone()));

                // Set up documents
                for i in 0..num_docs {
                    let doc_id = DocId::new(format!("doc_{}", i));
                    let source = format!("# Document {}\n\nContent for document {}.", i, i);
                    storage.set_document(doc_id, source);
                }

                b.iter(|| {
                    // Query all documents
                    for i in 0..num_docs {
                        let doc_id = DocId::new(format!("doc_{}", i));
                        let content =
                            db.query::<ExpandToContentQuery>(black_box(doc_id));
                        black_box(content);
                    }
                })
            },
        );
    }

    group.finish();
}

fn bench_early_cutoff(c: &mut Criterion) {
    let doc_id = DocId::new("test");
    let storage = Arc::new(SourceStorage::new());
    let db = Db::new();
    db.set_any("source_storage".to_string(), Box::new(storage.clone()));

    // Set initial content
    storage.set_document(doc_id.clone(), "# Test".to_string());
    let _initial = db.query::<ExpandToContentQuery>(doc_id.clone());

    c.bench_function("early_cutoff_benefit", |b| {
        b.iter(|| {
            // Change source but result is same (whitespace)
            storage.set_document(doc_id.clone(), "# Test  ".to_string());
            db.invalidate::<ParseShrubberyQuery>(doc_id.clone());

            // This should benefit from early cutoff
            let content = db.query::<ExpandToContentQuery>(black_box(doc_id.clone()));
            black_box(content);

            // Reset
            storage.set_document(doc_id.clone(), "# Test".to_string());
            db.invalidate::<ParseShrubberyQuery>(doc_id.clone());
        })
    });
}

criterion_group!(
    benches,
    bench_source_storage,
    bench_parse_query,
    bench_expand_query,
    bench_layout_query,
    bench_full_pipeline,
    bench_invalidation,
    bench_multiple_documents,
    bench_early_cutoff,
);

criterion_main!(benches);
