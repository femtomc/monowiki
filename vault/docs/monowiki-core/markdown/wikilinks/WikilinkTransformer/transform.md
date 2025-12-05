---
title: markdown::wikilinks::WikilinkTransformer::transform
description: null
summary: Transform events, converting [[wikilinks]] to HTML links
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:markdown::wikilinks::WikilinkTransformer
draft: false
updated: null
slug: markdown-wikilinks-wikilinktransformer-transform
permalink: null
aliases:
- markdown::wikilinks::WikilinkTransformer::transform
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

# markdown::wikilinks::WikilinkTransformer::transform

**Kind:** Method

**Source:** [monowiki-core/src/markdown/wikilinks.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/wikilinks.rs#L31)

```rust
pub fn transform(&self, events: Vec<Event<'_>>,) ->(Vec<Event<'static>>, Vec<String>, Vec<Diagnostic>)
```

Transform events, converting [[wikilinks]] to HTML links

Returns (transformed_events, outgoing_links)

## Reference source: [monowiki-core/src/markdown/wikilinks.rs L31–L98](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/wikilinks.rs#L31)

```rust
    /// Transform events, converting [[wikilinks]] to HTML links
    ///
    /// Returns (transformed_events, outgoing_links)
    pub fn transform(
        &self,
        events: Vec<Event<'_>>,
    ) -> (Vec<Event<'static>>, Vec<String>, Vec<Diagnostic>) {
        let mut result = Vec::new();
        let mut outgoing_links = Vec::new();
        let mut diagnostics = Vec::new();
        let mut i = 0;
        let mut in_code_block = false;

        while i < events.len() {
            // Track code block context
            match &events[i] {
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block = true;
                    result.push(events[i].clone().into_static());
                    i += 1;
                    continue;
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    result.push(events[i].clone().into_static());
                    i += 1;
                    continue;
                }
                _ => {}
            }

            // Skip wikilink processing inside code blocks
            if in_code_block {
                result.push(events[i].clone().into_static());
                i += 1;
                continue;
            }

            if let Event::Text(_) = &events[i] {
                // Collect all consecutive Text events and merge them
                let mut merged_text = String::new();

                while i < events.len() {
                    if let Event::Text(text) = &events[i] {
                        merged_text.push_str(text.as_ref());
                        i += 1;
                    } else {
                        break;
                    }
                }

                // Check if merged text contains wikilink syntax
                if merged_text.contains("[[") && merged_text.contains("]]") {
                    let (transformed, links, mut diags) = self.process_wikilinks(&merged_text);
                    result.extend(transformed);
                    outgoing_links.extend(links);
                    diagnostics.append(&mut diags);
                } else {
                    result.push(Event::Text(CowStr::Boxed(merged_text.into_boxed_str())));
                }
            } else {
                result.push(events[i].clone().into_static());
                i += 1;
            }
        }

        (result, outgoing_links, diagnostics)
    }
```
