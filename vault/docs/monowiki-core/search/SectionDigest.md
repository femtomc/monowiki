---
title: search::SectionDigest
description: null
summary: Lightweight digest for section-level change detection
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:search
draft: false
updated: null
slug: search-sectiondigest
permalink: null
aliases:
- search::SectionDigest
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

# search::SectionDigest

**Kind:** Struct

**Source:** [monowiki-core/src/search.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/search.rs#L228)

```rust
pub struct SectionDigest
```

Lightweight digest for section-level change detection

## Reference source: [monowiki-core/src/search.rs L228–L236](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/search.rs#L228)

```rust
/// Lightweight digest for section-level change detection
#[derive(Debug, Clone)]
pub struct SectionDigest {
    pub section_id: String,
    pub heading: String,
    pub hash: String,
    /// Original anchor id (heading slug) if available
    pub anchor_id: Option<String>,
}
```
