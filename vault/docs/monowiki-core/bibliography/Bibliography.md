---
title: bibliography::Bibliography
description: null
summary: Resolved bibliography entries for a single note.
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:bibliography
draft: false
updated: null
slug: bibliography-bibliography
permalink: null
aliases:
- bibliography::Bibliography
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

# bibliography::Bibliography

**Kind:** Struct

**Source:** [monowiki-core/src/bibliography.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L104)

```rust
pub struct Bibliography
```

Resolved bibliography entries for a single note.

## Reference source: [monowiki-core/src/bibliography.rs L104–L108](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L104)

```rust
/// Resolved bibliography entries for a single note.
#[derive(Debug, Clone, Default)]
pub struct Bibliography {
    entries: HashMap<String, Entry>,
}
```
