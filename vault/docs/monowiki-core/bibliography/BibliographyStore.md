---
title: bibliography::BibliographyStore
description: null
summary: Cached bibliography loader to avoid re-reading the same `.bib` files.
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:bibliography
draft: false
updated: null
slug: bibliography-bibliographystore
permalink: null
aliases:
- bibliography::BibliographyStore
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

# bibliography::BibliographyStore

**Kind:** Struct

**Source:** [monowiki-core/src/bibliography.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L13)

```rust
pub struct BibliographyStore
```

Cached bibliography loader to avoid re-reading the same `.bib` files.

## Reference source: [monowiki-core/src/bibliography.rs L13–L18](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L13)

```rust
/// Cached bibliography loader to avoid re-reading the same `.bib` files.
#[derive(Debug, Default)]
pub struct BibliographyStore {
    cache: HashMap<PathBuf, Library>,
    diagnostics: Vec<Diagnostic>,
}
```
