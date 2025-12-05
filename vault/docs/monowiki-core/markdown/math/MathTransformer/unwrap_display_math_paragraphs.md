---
title: markdown::math::MathTransformer::unwrap_display_math_paragraphs
description: null
summary: Second-pass transform to unwrap paragraphs containing display math
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:markdown::math::MathTransformer
draft: false
updated: null
slug: markdown-math-mathtransformer-unwrap-display-math-paragraphs
permalink: null
aliases:
- markdown::math::MathTransformer::unwrap_display_math_paragraphs
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

# markdown::math::MathTransformer::unwrap_display_math_paragraphs

**Kind:** Method

**Source:** [monowiki-core/src/markdown/math.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/math.rs#L115)

```rust
pub fn unwrap_display_math_paragraphs(&self, events: Vec<Event<'static>>,) -> Vec<Event<'static>>
```

Second-pass transform to unwrap paragraphs containing display math
This must run AFTER math rendering (which converts DisplayMath to Html)

## Reference source: [monowiki-core/src/markdown/math.rs L115–L185](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/math.rs#L115)

```rust
    /// Second-pass transform to unwrap paragraphs containing display math
    /// This must run AFTER math rendering (which converts DisplayMath to Html)
    pub fn unwrap_display_math_paragraphs(
        &self,
        events: Vec<Event<'static>>,
    ) -> Vec<Event<'static>> {
        let mut result = Vec::new();
        let mut i = 0;

        while i < events.len() {
            match &events[i] {
                Event::Start(Tag::Paragraph) => {
                    i += 1;
                    let mut paragraph: Vec<Event<'static>> = Vec::new();
                    while i < events.len() {
                        if matches!(events[i], Event::End(TagEnd::Paragraph)) {
                            break;
                        }
                        paragraph.push(events[i].clone());
                        i += 1;
                    }

                    // If the paragraph contains rendered display math (HTML), unwrap it
                    if self.paragraph_has_rendered_display_math(&paragraph) {
                        let mut buffer = Vec::new();
                        for ev in paragraph {
                            // Check if this is rendered display math HTML
                            let is_display_math = match &ev {
                                Event::Html(html) => html.contains("math-display"),
                                Event::InlineHtml(html) => html.contains("math-display"),
                                _ => false,
                            };

                            if is_display_math {
                                if !buffer.is_empty() {
                                    result.push(Event::Start(Tag::Paragraph));
                                    result.append(&mut buffer);
                                    result.push(Event::End(TagEnd::Paragraph));
                                }
                                result.push(ev);
                            } else {
                                buffer.push(ev);
                            }
                        }
                        if !buffer.is_empty() {
                            result.push(Event::Start(Tag::Paragraph));
                            result.append(&mut buffer);
                            result.push(Event::End(TagEnd::Paragraph));
                        }
                    } else {
                        result.push(Event::Start(Tag::Paragraph));
                        result.extend(paragraph);
                        if i < events.len() {
                            result.push(events[i].clone()); // End paragraph
                        }
                    }

                    // Skip the paragraph end (if present)
                    if i < events.len() {
                        i += 1;
                    }
                }
                _ => {
                    result.push(events[i].clone());
                    i += 1;
                }
            }
        }

        result
    }
```
