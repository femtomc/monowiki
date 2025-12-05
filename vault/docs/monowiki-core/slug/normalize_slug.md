---
title: slug::normalize_slug
description: null
summary: Normalize a slug (ensure it's properly formatted)
date: null
type: doc
tags:
- rust
- api
- kind:function
- module:slug
draft: false
updated: null
slug: slug-normalize-slug
permalink: null
aliases:
- slug::normalize_slug
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

# slug::normalize_slug

**Kind:** Function

**Source:** [monowiki-core/src/slug.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/slug.rs#L61)

```rust
pub fn normalize_slug(slug: &str) -> String
```

Normalize a slug (ensure it's properly formatted)

## Reference source: [monowiki-core/src/slug.rs L61–L64](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/slug.rs#L61)

```rust
/// Normalize a slug (ensure it's properly formatted)
pub fn normalize_slug(slug: &str) -> String {
    slugify(slug)
}
```
