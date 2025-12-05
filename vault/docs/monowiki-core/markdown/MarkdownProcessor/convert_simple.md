---
title: markdown::MarkdownProcessor::convert_simple
description: null
summary: Convert markdown to HTML without link tracking
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:markdown::MarkdownProcessor
draft: false
updated: null
slug: markdown-markdownprocessor-convert-simple
permalink: null
aliases:
- markdown::MarkdownProcessor::convert_simple
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

# markdown::MarkdownProcessor::convert_simple

**Kind:** Method

**Source:** [monowiki-core/src/markdown/mod.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/mod.rs#L144)

```rust
pub fn convert_simple(&self, markdown: &str) -> String
```

Convert markdown to HTML without link tracking

## Reference source: [monowiki-core/src/markdown/mod.rs L144–L149](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/mod.rs#L144)

```rust
    /// Convert markdown to HTML without link tracking
    pub fn convert_simple(&self, markdown: &str) -> String {
        let slug_map = HashMap::new();
        let (html, _, _, _) = self.convert(markdown, &slug_map, "/", None, None, None);
        html
    }
```
