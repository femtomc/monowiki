---
title: markdown::citations::render_references
description: null
summary: Render the collected references as an HTML list.
date: null
type: doc
tags:
- rust
- api
- kind:function
- module:markdown::citations
draft: false
updated: null
slug: markdown-citations-render-references
permalink: null
aliases:
- markdown::citations::render_references
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

# markdown::citations::render_references

**Kind:** Function

**Source:** [monowiki-core/src/markdown/citations.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/citations.rs#L163)

```rust
pub fn render_references(references: &[CitationRef]) -> Option<String>
```

Render the collected references as an HTML list.

## Reference source: [monowiki-core/src/markdown/citations.rs L163–L189](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/citations.rs#L163)

```rust
/// Render the collected references as an HTML list.
pub fn render_references(references: &[CitationRef]) -> Option<String> {
    if references.is_empty() {
        return None;
    }

    let mut html = String::from(
        r#"<section class="references"><h3>References</h3><ol class="reference-list">"#,
    );
    for cite in references {
        html.push_str(&format!(r#"<li id="ref-{}">"#, cite.number));
        let body = cite
            .entry
            .as_ref()
            .map(|entry| format_entry(entry))
            .unwrap_or_else(|| format!("Missing entry: {}", html_escape(&cite.key)));
        html.push_str(&body);
        html.push_str(&format!(
            " <a class=\"ref-backlink\" href=\"#cite-{}\" aria-label=\"Back to citation\">&#8617;</a>",
            cite.number
        ));
        html.push_str("</li>");
    }
    html.push_str("</ol></section>");

    Some(html)
}
```
