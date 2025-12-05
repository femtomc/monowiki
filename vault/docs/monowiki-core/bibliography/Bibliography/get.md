---
title: bibliography::Bibliography::get
description: null
summary: Lookup a bibliography entry by key.
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:bibliography::Bibliography
draft: false
updated: null
slug: bibliography-bibliography-get
permalink: null
aliases:
- bibliography::Bibliography::get
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

# bibliography::Bibliography::get

**Kind:** Method

**Source:** [monowiki-core/src/bibliography.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L111)

```rust
pub fn get(&self, key: &str) -> Option<&Entry>
```

Lookup a bibliography entry by key.

## Reference source: [monowiki-core/src/bibliography.rs L111–L114](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L111)

```rust
    /// Lookup a bibliography entry by key.
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.get(key)
    }
```
