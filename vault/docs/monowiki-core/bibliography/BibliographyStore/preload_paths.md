---
title: bibliography::BibliographyStore::preload_paths
description: null
summary: Ensure the given paths are loaded into the cache.
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:bibliography::BibliographyStore
draft: false
updated: null
slug: bibliography-bibliographystore-preload-paths
permalink: null
aliases:
- bibliography::BibliographyStore::preload_paths
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

# bibliography::BibliographyStore::preload_paths

**Kind:** Method

**Source:** [monowiki-core/src/bibliography.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L28)

```rust
pub fn preload_paths(&mut self, paths: &[PathBuf])
```

Ensure the given paths are loaded into the cache.

## Reference source: [monowiki-core/src/bibliography.rs L28–L33](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L28)

```rust
    /// Ensure the given paths are loaded into the cache.
    pub fn preload_paths(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.ensure_loaded(path);
        }
    }
```
