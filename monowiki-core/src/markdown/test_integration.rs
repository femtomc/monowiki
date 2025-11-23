//! Integration tests for markdown processing

use super::*;
use std::collections::HashMap;

#[test]
fn test_full_pipeline_with_wikilinks() {
    let markdown = "Check out [[Rust Safety]] for more info.";
    let mut slug_map = HashMap::new();
    slug_map.insert("rust-safety".to_string(), "Rust Safety".to_string());

    let processor = MarkdownProcessor::new();
    let (html, links, toc) = processor.convert(markdown, &slug_map, "/");

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
    let (html, links, toc) = processor.convert(markdown, &slug_map, "/");

    println!("HTML: {}", html);
    println!("Links: {:?}", links);

    // The link should be converted
    assert!(!html.contains("[["), "Wikilinks should be converted");
    assert!(html.contains("<a href"), "Should contain a link");
    assert!(toc.is_none());
}
