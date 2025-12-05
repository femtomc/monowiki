---
title: markdown::MarkdownProcessor::convert
description: null
summary: Convert markdown to HTML with all custom transforms
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:markdown::MarkdownProcessor
draft: false
updated: null
slug: markdown-markdownprocessor-convert
permalink: null
aliases:
- markdown::MarkdownProcessor::convert
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

# markdown::MarkdownProcessor::convert

**Kind:** Method

**Source:** [monowiki-core/src/markdown/mod.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/mod.rs#L57)

```rust
pub fn convert(&self, markdown: &str, slug_map: &HashMap<String, String>, base_url: &str, citation_context: Option<&CitationContext>, note_slug: Option<&str>, source_path: Option<&str>,) ->(String, Vec<String>, Option<String>, Vec<Diagnostic>)
```

Convert markdown to HTML with all custom transforms

Returns a tuple of (html, outgoing_links, toc_html, diagnostics)

## Reference source: [monowiki-core/src/markdown/mod.rs L57–L142](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/mod.rs#L57)

```rust
    /// Convert markdown to HTML with all custom transforms
    ///
    /// Returns a tuple of (html, outgoing_links, toc_html, diagnostics)
    pub fn convert(
        &self,
        markdown: &str,
        slug_map: &HashMap<String, String>,
        base_url: &str,
        citation_context: Option<&CitationContext>,
        note_slug: Option<&str>,
        source_path: Option<&str>,
    ) -> (String, Vec<String>, Option<String>, Vec<Diagnostic>) {
        // Parse markdown into events
        let parser = Parser::new_ext(markdown, self.options);
        let events: Vec<Event> = parser.collect();

        // Collect headings for TOC and later ID injection
        let headings = collect_headings(&events);
        let mut diagnostics: Vec<Diagnostic> = Vec::new();

        // Transform math delimiters and render to MathJax HTML
        let math_transformer = MathTransformer::new();
        let events = math_transformer.transform(events);

        // Apply nota blocks (needs paragraph structure intact)
        let nota_transformer = NotaBlockTransformer::new();
        let events = nota_transformer.transform(events);

        // Unwrap paragraphs with display math
        let events = math_transformer.unwrap_display_math_paragraphs(events);

        // Apply sidenote transform
        let sidenote_transformer = SidenoteTransformer::new();
        let events = sidenote_transformer.transform(events);

        // Apply wikilink transform
        let wikilink_transformer = WikilinkTransformer::new(
            slug_map,
            base_url,
            note_slug.map(|s| s.to_string()),
            source_path.map(|s| s.to_string()),
        );
        let (events, outgoing_links, mut link_diags) = wikilink_transformer.transform(events);
        diagnostics.append(&mut link_diags);

        // Apply citation transform
        let mut citation_references = Vec::new();
        let events = if let Some(ctx) = citation_context {
            let transformer = CitationTransformer::new(
                ctx,
                note_slug.map(|s| s.to_string()),
                source_path.map(|s| s.to_string()),
            );
            let (events, refs, mut cite_diags) = transformer.transform(events);
            citation_references = refs;
            diagnostics.append(&mut cite_diags);
            events
        } else {
            events
        };

        // Inject heading ids to match TOC anchors
        let events = attach_heading_ids(events, &headings);
        let events = add_heading_anchors(events);

        // Apply syntax highlighting to code blocks
        let highlight_transformer = HighlightTransformer::new();
        let events = highlight_transformer.transform(events);

        // Convert events to HTML
        let mut html_output = String::new();
        html::push_html(&mut html_output, events.into_iter());

        if let Some(refs_html) = render_references(&citation_references) {
            html_output.push('\n');
            html_output.push_str(&refs_html);
        }

        let toc_html = if headings.is_empty() {
            None
        } else {
            Some(render_toc(&headings))
        };

        (html_output, outgoing_links, toc_html, diagnostics)
    }
```
