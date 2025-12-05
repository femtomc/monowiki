---
title: markdown::citations::CitationRef
description: null
summary: A single reference entry in the rendered bibliography.
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:markdown::citations
draft: false
updated: null
slug: markdown-citations-citationref
permalink: null
aliases:
- markdown::citations::CitationRef
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

# markdown::citations::CitationRef

**Kind:** Struct

**Source:** [monowiki-core/src/markdown/citations.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/citations.rs#L18)

```rust
pub struct CitationRef
```

A single reference entry in the rendered bibliography.

## Reference source: [monowiki-core/src/markdown/citations.rs L18–L24](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/citations.rs#L18)

```rust
/// A single reference entry in the rendered bibliography.
#[derive(Debug, Clone)]
pub struct CitationRef {
    pub key: String,
    pub number: usize,
    pub entry: Option<Entry>,
}
```
