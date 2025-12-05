---
title: bibliography::Bibliography::is_empty
description: null
summary: Returns true if there are no entries.
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:bibliography::Bibliography
draft: false
updated: null
slug: bibliography-bibliography-is-empty
permalink: null
aliases:
- bibliography::Bibliography::is_empty
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

# bibliography::Bibliography::is_empty

**Kind:** Method

**Source:** [monowiki-core/src/bibliography.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L116)

```rust
pub fn is_empty(&self) -> bool
```

Returns true if there are no entries.

## Reference source: [monowiki-core/src/bibliography.rs L116–L119](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L116)

```rust
    /// Returns true if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
```
