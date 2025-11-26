//! Benchmarks for the incremental query system

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use monowiki_incremental::prelude::*;
use monowiki_incremental::queries::{
    ExpandToContentQuery, LayoutSectionQuery, ParseShrubberyQuery, SourceTextQuery,
};
use monowiki_incremental::queries::layout::Viewport;
use monowiki_incremental::invalidation::BlockId;

fn bench_source_text_query(c: &mut Criterion) {
    let db = Db::new();
    let section_id = SectionId(BlockId(1).0);

    c.bench_function("source_text_set", |b| {
        b.iter(|| {
            SourceTextQuery::set(
                &db,
                section_id,
                black_box("# Test Heading\n\nParagraph.".to_string()),
            );
        })
    });

    SourceTextQuery::set(&db, section_id, "# Test".to_string());

    c.bench_function("source_text_query", |b| {
        b.iter(|| {
            let text = db.query::<SourceTextQuery>(black_box(section_id));
            black_box(text);
        })
    });
}

fn bench_parse_query(c: &mut Criterion) {
    let db = Db::new();
    let section_id = SectionId(BlockId(1).0);

    let source = "# Heading\n\nParagraph text.\n\n## Subheading\n\nMore text.";
    SourceTextQuery::set(&db, section_id, source.to_string());

    c.bench_function("parse_query_cold", |b| {
        b.iter(|| {
            db.clear_all();
            SourceTextQuery::set(&db, section_id, source.to_string());
            let shrubbery = db.query::<ParseShrubberyQuery>(black_box(section_id));
            black_box(shrubbery);
        })
    });

    c.bench_function("parse_query_hot", |b| {
        b.iter(|| {
            let shrubbery = db.query::<ParseShrubberyQuery>(black_box(section_id));
            black_box(shrubbery);
        })
    });
}

fn bench_expand_query(c: &mut Criterion) {
    let db = Db::new();
    let section_id = SectionId(BlockId(1).0);

    let source = "# Heading\n\nParagraph.\n\n```rust\nfn main() {}\n```";
    SourceTextQuery::set(&db, section_id, source.to_string());

    c.bench_function("expand_query_cold", |b| {
        b.iter(|| {
            db.clear_all();
            SourceTextQuery::set(&db, section_id, source.to_string());
            let content = db.query::<ExpandToContentQuery>(black_box(section_id));
            black_box(content);
        })
    });

    c.bench_function("expand_query_hot", |b| {
        b.iter(|| {
            let content = db.query::<ExpandToContentQuery>(black_box(section_id));
            black_box(content);
        })
    });
}

fn bench_layout_query(c: &mut Criterion) {
    let db = Db::new();
    let section_id = SectionId(BlockId(1).0);
    let viewport = Viewport::new(800, 600);

    let source = "# Title\n\nParagraph.\n\n## Section\n\nMore text.";
    SourceTextQuery::set(&db, section_id, source.to_string());

    c.bench_function("layout_query_cold", |b| {
        b.iter(|| {
            db.clear_all();
            SourceTextQuery::set(&db, section_id, source.to_string());
            let layout =
                db.query::<LayoutSectionQuery>(black_box((section_id, viewport)));
            black_box(layout);
        })
    });

    c.bench_function("layout_query_hot", |b| {
        b.iter(|| {
            let layout =
                db.query::<LayoutSectionQuery>(black_box((section_id, viewport)));
            black_box(layout);
        })
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    let db = Db::new();
    let section_id = SectionId(BlockId(1).0);
    let viewport = Viewport::new(800, 600);

    let source = "# Introduction\n\nThis is a test document with some content.";

    c.bench_function("full_pipeline_cold", |b| {
        b.iter(|| {
            db.clear_all();
            SourceTextQuery::set(&db, section_id, source.to_string());

            let _shrubbery = db.query::<ParseShrubberyQuery>(section_id);
            let _content = db.query::<ExpandToContentQuery>(section_id);
            let layout = db.query::<LayoutSectionQuery>((section_id, viewport));
            black_box(layout);
        })
    });

    SourceTextQuery::set(&db, section_id, source.to_string());

    c.bench_function("full_pipeline_hot", |b| {
        b.iter(|| {
            let _shrubbery = db.query::<ParseShrubberyQuery>(black_box(section_id));
            let _content = db.query::<ExpandToContentQuery>(black_box(section_id));
            let layout =
                db.query::<LayoutSectionQuery>(black_box((section_id, viewport)));
            black_box(layout);
        })
    });
}

fn bench_invalidation(c: &mut Criterion) {
    let db = Db::new();
    let section_id = SectionId(BlockId(1).0);
    let viewport = Viewport::new(800, 600);

    c.bench_function("invalidate_and_recompute", |b| {
        let mut counter = 0;
        b.iter(|| {
            // Change source slightly
            let source = format!("# Version {}\n\nContent.", counter);
            SourceTextQuery::set(&db, section_id, source);

            // Recompute pipeline
            let layout = db.query::<LayoutSectionQuery>((section_id, viewport));
            black_box(layout);

            counter += 1;
        })
    });
}

fn bench_multiple_sections(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_sections");

    for num_sections in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_sections),
            num_sections,
            |b, &num_sections| {
                let db = Db::new();

                // Set up sections
                for i in 0..num_sections {
                    let section_id = SectionId(BlockId(i).0);
                    let source = format!("# Section {}\n\nContent for section {}.", i, i);
                    SourceTextQuery::set(&db, section_id, source);
                }

                b.iter(|| {
                    // Query all sections
                    for i in 0..num_sections {
                        let section_id = SectionId(BlockId(i).0);
                        let content =
                            db.query::<ExpandToContentQuery>(black_box(section_id));
                        black_box(content);
                    }
                })
            },
        );
    }

    group.finish();
}

fn bench_early_cutoff(c: &mut Criterion) {
    let db = Db::new();
    let section_id = SectionId(BlockId(1).0);

    // Set initial content
    SourceTextQuery::set(&db, section_id, "# Test".to_string());
    let _initial = db.query::<ExpandToContentQuery>(section_id);

    c.bench_function("early_cutoff_benefit", |b| {
        b.iter(|| {
            // Change source but result is same (whitespace)
            SourceTextQuery::set(&db, section_id, "# Test  ".to_string());

            // This should benefit from early cutoff
            let content = db.query::<ExpandToContentQuery>(black_box(section_id));
            black_box(content);

            // Reset
            SourceTextQuery::set(&db, section_id, "# Test".to_string());
        })
    });
}

criterion_group!(
    benches,
    bench_source_text_query,
    bench_parse_query,
    bench_expand_query,
    bench_layout_query,
    bench_full_pipeline,
    bench_invalidation,
    bench_multiple_sections,
    bench_early_cutoff,
);

criterion_main!(benches);
