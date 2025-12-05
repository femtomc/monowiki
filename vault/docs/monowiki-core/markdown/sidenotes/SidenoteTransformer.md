---
title: markdown::sidenotes::SidenoteTransformer
description: null
summary: Transformer for sidenote syntax
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:markdown::sidenotes
draft: false
updated: null
slug: markdown-sidenotes-sidenotetransformer
permalink: null
aliases:
- markdown::sidenotes::SidenoteTransformer
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

# markdown::sidenotes::SidenoteTransformer

**Kind:** Struct

**Source:** [monowiki-core/src/markdown/sidenotes.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/sidenotes.rs#L5)

```rust
pub struct SidenoteTransformer
```

Transformer for sidenote syntax

## Reference source: [monowiki-core/src/markdown/sidenotes.rs L5–L8](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/sidenotes.rs#L5)

```rust
/// Transformer for sidenote syntax
pub struct SidenoteTransformer {
    counter: std::cell::Cell<usize>,
}
```
