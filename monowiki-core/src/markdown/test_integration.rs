//! Integration tests for markdown processing

use super::*;
use std::collections::HashMap;

#[test]
fn test_full_pipeline_with_wikilinks() {
    let markdown = "Check out [[Rust Safety]] for more info.";
    let mut slug_map = HashMap::new();
    slug_map.insert("rust-safety".to_string(), "Rust Safety".to_string());

    let processor = MarkdownProcessor::new();
    let (html, links, toc) = processor.convert(markdown, &slug_map, "/", None);

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
    let (html, links, toc) = processor.convert(markdown, &slug_map, "/", None);

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
    let (html, _links, _toc) = processor.convert(
        markdown,
        &slug_map,
        "/",
        Some("#let foo = 42"),
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
    let (html, links, _toc) = processor.convert(markdown, &slug_map, "/", None);

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
