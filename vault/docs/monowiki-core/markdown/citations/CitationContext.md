---
title: markdown::citations::CitationContext
description: null
summary: Context required to resolve citation keys.
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:markdown::citations
draft: false
updated: null
slug: markdown-citations-citationcontext
permalink: null
aliases:
- markdown::citations::CitationContext
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

# markdown::citations::CitationContext

**Kind:** Struct

**Source:** [monowiki-core/src/markdown/citations.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/citations.rs#L13)

```rust
pub struct CitationContext<'a>
```

Context required to resolve citation keys.

## Reference source: [monowiki-core/src/markdown/citations.rs L13–L16](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/citations.rs#L13)

```rust
/// Context required to resolve citation keys.
pub struct CitationContext<'a> {
    pub bibliography: &'a Bibliography,
}
```
