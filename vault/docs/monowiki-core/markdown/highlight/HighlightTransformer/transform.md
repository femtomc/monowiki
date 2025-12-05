---
title: markdown::highlight::HighlightTransformer::transform
description: null
summary: Transform events, adding syntax highlighting to code blocks
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:markdown::highlight::HighlightTransformer
draft: false
updated: null
slug: markdown-highlight-highlighttransformer-transform
permalink: null
aliases:
- markdown::highlight::HighlightTransformer::transform
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

# markdown::highlight::HighlightTransformer::transform

**Kind:** Method

**Source:** [monowiki-core/src/markdown/highlight.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/highlight.rs#L62)

```rust
pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>>
```

Transform events, adding syntax highlighting to code blocks

## Reference source: [monowiki-core/src/markdown/highlight.rs L62–L105](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/highlight.rs#L62)

```rust
    /// Transform events, adding syntax highlighting to code blocks
    pub fn transform(&self, events: Vec<Event<'_>>) -> Vec<Event<'static>> {
        let mut result = Vec::new();
        let mut in_code_block = false;
        let mut code_info: Option<CodeBlockInfo> = None;
        let mut code_content = String::new();

        for event in events {
            match event {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                    in_code_block = true;
                    code_info = Some(CodeBlockInfo::parse(&lang));
                    code_content.clear();
                }
                Event::Text(text) if in_code_block => {
                    code_content.push_str(text.as_ref());
                }
                Event::End(TagEnd::CodeBlock) if in_code_block => {
                    in_code_block = false;

                    // Highlight the code
                    if let Some(info) = &code_info {
                        let highlighted =
                            self.highlight_code(&code_content, &info.lang, info.title.as_deref());
                        result.push(Event::Html(CowStr::Boxed(highlighted.into_boxed_str())));
                    } else {
                        // No language specified, output as plain pre/code
                        result.push(Event::Start(Tag::CodeBlock(CodeBlockKind::Indented)));
                        result.push(Event::Text(CowStr::Boxed(
                            code_content.clone().into_boxed_str(),
                        )));
                        result.push(Event::End(TagEnd::CodeBlock));
                    }

                    code_info = None;
                }
                _ => {
                    result.push(self.event_into_static(event));
                }
            }
        }

        result
    }
```
