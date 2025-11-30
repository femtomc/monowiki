//! Integration tests for markdown processing

use super::*;
use crate::bibliography::BibliographyStore;
use std::collections::HashMap;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_full_pipeline_with_wikilinks() {
    let markdown = "Check out [[Rust Safety]] for more info.";
    let mut slug_map = HashMap::new();
    slug_map.insert("rust-safety".to_string(), "Rust Safety".to_string());

    let processor = MarkdownProcessor::new();
    let (html, links, toc, _) = processor.convert(markdown, &slug_map, "/", None, None, None, None);

    println!("Input: {}", markdown);
    println!("Output: {}", html);
    println!("Links: {:?}", links);

    assert!(html.contains("<a href"), "Should contain a link");
    assert!(
        links.contains(&"rust-safety".to_string()),
        "Should track the link"
    );
    assert!(toc.is_none());
}

#[test]
fn test_wikilink_in_paragraph() {
    let markdown = "This is a paragraph with [[Page Name]] in it.";
    let slug_map = HashMap::new();

    let processor = MarkdownProcessor::new();
    let (html, links, toc, _) = processor.convert(markdown, &slug_map, "/", None, None, None, None);

    println!("HTML: {}", html);
    println!("Links: {:?}", links);

    // The link should be converted
    assert!(!html.contains("[["), "Wikilinks should be converted");
    assert!(html.contains("<a href"), "Should contain a link");
    assert!(toc.is_none());
}

#[test]
fn test_typst_preamble_applied() {
    let markdown = "$$ #foo $$";
    let slug_map = HashMap::new();
    let processor = MarkdownProcessor::new();
    let (html, _links, _toc, _) = processor.convert(
        markdown,
        &slug_map,
        "/",
        Some("#let foo = 42"),
        None,
        None,
        None,
    );

    assert!(
        html.contains("typst-display"),
        "Typst preamble should allow rendering math blocks"
    );
}

#[test]
fn test_nota_block_transformer_wraps_paragraph() {
    let markdown = "@Definition[label=sem]{Small-step semantics}: See [[Eval]].";
    let mut slug_map = HashMap::new();
    slug_map.insert("eval".to_string(), "/eval".to_string());
    let processor = MarkdownProcessor::new();
    let (html, links, _toc, _) =
        processor.convert(markdown, &slug_map, "/", None, None, None, None);

    assert!(
        html.contains("nota-block nota-definition"),
        "Custom Nota-like block wrapper should be present"
    );
    assert!(html.contains("id=\"sem\""), "Custom id should be applied");
    assert!(
        links.contains(&"eval".to_string()),
        "Wikilinks inside custom blocks should still be resolved"
    );
}

#[test]
fn test_citations_render_references() {
    let bibtex = r#"
@article{knuth1990,
  title = {Literate Programming},
  author = {Knuth, Donald E.},
  date = {1990},
  url = {https://example.com}
}
"#;

    let mut tmp = NamedTempFile::new().unwrap();
    write!(tmp, "{}", bibtex).unwrap();

    let mut store = BibliographyStore::new();
    let bibliography = store.collect(&vec![tmp.path().to_path_buf()]);
    let ctx = citations::CitationContext {
        bibliography: &bibliography,
    };

    let slug_map = HashMap::new();
    let processor = MarkdownProcessor::new();
    let (html, _links, _toc, _) = processor.convert(
        "See [@knuth1990] for details.",
        &slug_map,
        "/",
        None,
        Some(&ctx),
        None,
        None,
    );

    assert!(
        html.contains("References"),
        "Should append a reference list when citations exist"
    );
    assert!(html.contains("cite-1"), "Inline citation anchor rendered");
    assert!(html.contains("ref-1"), "Reference target rendered");
    assert!(
        html.contains("Knuth"),
        "Reference entry should include author information"
    );
}
