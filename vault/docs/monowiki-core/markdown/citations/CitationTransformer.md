---
title: markdown::citations::CitationTransformer
description: null
summary: Transform markdown events by replacing `[@key]` markers with inline citations.
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:markdown::citations
draft: false
updated: null
slug: markdown-citations-citationtransformer
permalink: null
aliases:
- markdown::citations::CitationTransformer
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

# markdown::citations::CitationTransformer

**Kind:** Struct

**Source:** [monowiki-core/src/markdown/citations.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/citations.rs#L26)

```rust
pub struct CitationTransformer<'a>
```

Transform markdown events by replacing `[@key]` markers with inline citations.

## Reference source: [monowiki-core/src/markdown/citations.rs L26–L33](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/citations.rs#L26)

```rust
/// Transform markdown events by replacing `[@key]` markers with inline citations.
pub struct CitationTransformer<'a> {
    ctx: &'a CitationContext<'a>,
    order: Vec<String>,
    index: HashMap<String, usize>,
    note_slug: Option<String>,
    source_path: Option<String>,
}
```
