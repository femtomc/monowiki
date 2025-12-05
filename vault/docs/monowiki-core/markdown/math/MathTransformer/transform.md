---
title: markdown::math::MathTransformer::transform
description: null
summary: 'Transform events: detect math delimiters and convert to MathJax HTML'
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:markdown::math::MathTransformer
draft: false
updated: null
slug: markdown-math-mathtransformer-transform
permalink: null
aliases:
- markdown::math::MathTransformer::transform
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

# markdown::math::MathTransformer::transform

**Kind:** Method

**Source:** [monowiki-core/src/markdown/math.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/math.rs#L13)

```rust
pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>>
```

Transform events: detect math delimiters and convert to MathJax HTML

Detects $$, $, \[...\], and \(...\) delimiters

## Reference source: [monowiki-core/src/markdown/math.rs L13–L113](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/math.rs#L13)

```rust
    /// Transform events: detect math delimiters and convert to MathJax HTML
    ///
    /// Detects $$, $, \[...\], and \(...\) delimiters
    pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>> {
        // First pass: convert events to static and find math delimiters
        let mut static_events: Vec<Event<'static>> = Vec::new();
        let mut open: Option<OpenMath> = None;
        let mut in_code_block = false;
        let mut in_inline_code = false;

        for event in events {
            let owned = self.event_into_static(event);

            // Track code blocks and inline code to skip math processing
            match &owned {
                Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
                Event::End(TagEnd::CodeBlock) => in_code_block = false,
                Event::Code(_) => in_inline_code = true,
                _ => in_inline_code = false,
            }

            // Skip math processing inside code blocks or inline code
            if in_code_block || in_inline_code {
                static_events.push(owned);
                continue;
            }

            if let Some(mut state) = open.take() {
                // We're in an open math delimiter, look for the closer
                match owned {
                    Event::Text(text) => {
                        let text_str = text.into_string();
                        if let Some(close_idx) = text_str.find(state.close) {
                            // Found the closer!
                            state.content.push_str(&text_str[..close_idx]);
                            // Render to MathJax HTML
                            let html = render_mathjax(&state.content, state.kind);
                            static_events.push(match state.kind {
                                DelimKind::Display => {
                                    Event::Html(CowStr::Boxed(html.into_boxed_str()))
                                }
                                DelimKind::Inline => {
                                    Event::InlineHtml(CowStr::Boxed(html.into_boxed_str()))
                                }
                            });

                            // Process any remaining text after the closer
                            let remaining = text_str[close_idx + state.close.len()..].to_string();
                            if !remaining.is_empty() {
                                self.process_text_for_math(
                                    remaining,
                                    &mut static_events,
                                    &mut open,
                                );
                            }
                        } else {
                            // Closer not found, accumulate content
                            state.content.push_str(&text_str);
                            open = Some(state);
                        }
                    }
                    Event::SoftBreak => {
                        state.content.push('\n');
                        open = Some(state);
                    }
                    Event::HardBreak => {
                        state.content.push_str("\n\n");
                        open = Some(state);
                    }
                    other => {
                        // Unexpected event while in math mode - emit accumulated content as literal and the event
                        static_events.push(Event::Text(CowStr::Boxed(
                            format!("{}{}", state.open, state.content).into_boxed_str(),
                        )));
                        static_events.push(other);
                    }
                }
            } else {
                // Not in math mode
                match owned {
                    Event::Text(text) => {
                        self.process_text_for_math(
                            text.into_string(),
                            &mut static_events,
                            &mut open,
                        );
                    }
                    other => static_events.push(other),
                }
            }
        }

        // If we still have an open math delimiter at the end, emit it as literal text
        if let Some(state) = open {
            static_events.push(Event::Text(CowStr::Boxed(
                format!("{}{}", state.open, state.content).into_boxed_str(),
            )));
        }

        static_events
    }
```
