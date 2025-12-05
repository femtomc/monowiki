---
title: bibliography::BibliographyStore::take_diagnostics
description: null
summary: Take accumulated diagnostics (clearing the internal buffer).
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:bibliography::BibliographyStore
draft: false
updated: null
slug: bibliography-bibliographystore-take-diagnostics
permalink: null
aliases:
- bibliography::BibliographyStore::take_diagnostics
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

# bibliography::BibliographyStore::take_diagnostics

**Kind:** Method

**Source:** [monowiki-core/src/bibliography.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L98)

```rust
pub fn take_diagnostics(&mut self) -> Vec<Diagnostic>
```

Take accumulated diagnostics (clearing the internal buffer).

## Reference source: [monowiki-core/src/bibliography.rs L98–L101](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L98)

```rust
    /// Take accumulated diagnostics (clearing the internal buffer).
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        mem::take(&mut self.diagnostics)
    }
```
