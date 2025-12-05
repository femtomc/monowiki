---
title: markdown::wikilinks::WikilinkTransformer
description: null
summary: Transformer for wikilink syntax
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:markdown::wikilinks
draft: false
updated: null
slug: markdown-wikilinks-wikilinktransformer
permalink: null
aliases:
- markdown::wikilinks::WikilinkTransformer
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

# markdown::wikilinks::WikilinkTransformer

**Kind:** Struct

**Source:** [monowiki-core/src/markdown/wikilinks.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/wikilinks.rs#L8)

```rust
pub struct WikilinkTransformer<'a>
```

Transformer for wikilink syntax

## Reference source: [monowiki-core/src/markdown/wikilinks.rs L8–L14](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/markdown/wikilinks.rs#L8)

```rust
/// Transformer for wikilink syntax
pub struct WikilinkTransformer<'a> {
    slug_map: &'a HashMap<String, String>,
    base_url: String,
    note_slug: Option<String>,
    source_path: Option<String>,
}
```
