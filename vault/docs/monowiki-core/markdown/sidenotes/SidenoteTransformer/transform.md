---
title: markdown::sidenotes::SidenoteTransformer::transform
description: null
summary: 'Transform events, converting [^sidenote: text] to HTML spans'
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:markdown::sidenotes::SidenoteTransformer
draft: false
updated: null
slug: markdown-sidenotes-sidenotetransformer-transform
permalink: null
aliases:
- markdown::sidenotes::SidenoteTransformer::transform
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

# markdown::sidenotes::SidenoteTransformer::transform

**Kind:** Method

**Source:** [monowiki-core/src/markdown/sidenotes.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/sidenotes.rs#L17)

```rust
pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>>
```

Transform events, converting [^sidenote: text] to HTML spans

## Reference source: [monowiki-core/src/markdown/sidenotes.rs L17–L74](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/sidenotes.rs#L17)

```rust
    /// Transform events, converting [^sidenote: text] to HTML spans
    pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>> {
        let mut result = Vec::new();
        let mut in_code_block = false;
        let mut i = 0;
        let events: Vec<_> = events.into_iter().collect();

        while i < events.len() {
            // Track code block context
            match &events[i] {
                Event::Start(Tag::CodeBlock(_)) => {
                    in_code_block = true;
                    result.push(self.event_into_static(events[i].clone()));
                    i += 1;
                    continue;
                }
                Event::End(TagEnd::CodeBlock) => {
                    in_code_block = false;
                    result.push(self.event_into_static(events[i].clone()));
                    i += 1;
                    continue;
                }
                _ => {}
            }

            // Skip sidenote processing inside code blocks
            if in_code_block {
                result.push(self.event_into_static(events[i].clone()));
                i += 1;
                continue;
            }

            if let Event::Text(_) = &events[i] {
                // Merge consecutive Text events (pulldown-cmark splits [^sidenote:] across events)
                let mut merged_text = String::new();
                while i < events.len() {
                    if let Event::Text(text) = &events[i] {
                        merged_text.push_str(text.as_ref());
                        i += 1;
                    } else {
                        break;
                    }
                }

                // Check if merged text contains sidenote syntax
                if merged_text.contains("[^sidenote:") && merged_text.contains("]") {
                    result.extend(self.process_sidenotes(&merged_text));
                } else {
                    result.push(Event::Text(CowStr::Boxed(merged_text.into_boxed_str())));
                }
            } else {
                result.push(self.event_into_static(events[i].clone()));
                i += 1;
            }
        }

        result
    }
```
