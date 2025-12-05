---
title: markdown::typst_math::TypstMathRenderer
description: null
summary: Render math events into inline SVG using Typst.
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:markdown::typst_math
draft: false
updated: null
slug: markdown-typst-math-typstmathrenderer
permalink: null
aliases:
- markdown::typst_math::TypstMathRenderer
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

# markdown::typst_math::TypstMathRenderer

**Kind:** Struct

**Source:** [monowiki-core/src/markdown/typst_math.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/typst_math.rs#L20)

```rust
pub struct TypstMathRenderer
```

Render math events into inline SVG using Typst.

## Reference source: [monowiki-core/src/markdown/typst_math.rs L20–L25](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/typst_math.rs#L20)

```rust
/// Render math events into inline SVG using Typst.
#[derive(Debug)]
pub struct TypstMathRenderer {
    fonts: Vec<&'static [u8]>,
    cache: Mutex<LruCache<String, String>>,
}
```
