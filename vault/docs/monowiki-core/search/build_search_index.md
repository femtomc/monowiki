---
title: search::build_search_index
description: null
summary: Build a granular search index from note HTML
date: null
type: doc
tags:
- rust
- api
- kind:function
- module:search
draft: false
updated: null
slug: search-build-search-index
permalink: null
aliases:
- search::build_search_index
typst_preamble: null
bibliography: []
target_slug: null
target_anchor: null
git_ref: null
quote: null
author: null
status: null
parent_id: null
---

# search::build_search_index

**Kind:** Function

**Source:** [monowiki-core/src/search.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/search.rs#L24)

```rust
pub fn build_search_index(slug: &str, title: &str, content_html: &str, tags: &[String], doc_type: &str, base_url: &str,) -> Vec<SearchEntry>
```

Build a granular search index from note HTML

## Reference source: [monowiki-core/src/search.rs L24–L100](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/search.rs#L24)

```rust
/// Build a granular search index from note HTML
pub fn build_search_index(
    slug: &str,
    title: &str,
    content_html: &str,
    tags: &[String],
    doc_type: &str,
    base_url: &str,
) -> Vec<SearchEntry> {
    // Extract plain text from HTML
    let plain_text = html_to_text(content_html);

    // Parse into sections based on heading markers in HTML
    let sections = extract_sections_from_html(content_html);

    if sections.is_empty() {
        let section_hash = compute_section_hash(&plain_text);
        let section_id = format!("{}-{}", slug, &section_hash[..8]);
        // Fallback: single entry for whole document
        let snippet = create_snippet(&plain_text, 200);
        return vec![SearchEntry {
            id: slug.to_string(),
            url: format!("{}{}.html", base_url, slug),
            section_id,
            section_hash,
            title: title.to_string(),
            section_title: String::new(),
            content: plain_text,
            snippet,
            tags: tags.to_vec(),
            doc_type: doc_type.to_string(),
        }];
    }

    // Create search entry for each section
    sections
        .into_iter()
        .map(|(heading, heading_id, section_text)| {
            let section_id = if heading_id.is_empty() {
                slug.to_string()
            } else {
                format!("{}#{}", slug, heading_id)
            };

            let url = if heading_id.is_empty() {
                format!("{}{}.html", base_url, slug)
            } else {
                format!("{}{}.html#{}", base_url, slug, heading_id)
            };

            let snippet = create_snippet(&section_text, 200);
            let section_hash = compute_section_hash(&section_text);
            let stable_section_id = format!(
                "{}-{}",
                if heading_id.is_empty() {
                    slug.to_string()
                } else {
                    heading_id.clone()
                },
                &section_hash[..8]
            );

            SearchEntry {
                id: section_id,
                section_id: stable_section_id,
                section_hash,
                url,
                title: title.to_string(),
                section_title: heading,
                content: section_text,
                snippet,
                tags: tags.to_vec(),
                doc_type: doc_type.to_string(),
            }
        })
        .collect()
}
```
