---
title: bibliography::BibliographyStore::collect
description: null
summary: Build a merged bibliography for the provided list of paths.
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:bibliography::BibliographyStore
draft: false
updated: null
slug: bibliography-bibliographystore-collect
permalink: null
aliases:
- bibliography::BibliographyStore::collect
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

# bibliography::BibliographyStore::collect

**Kind:** Method

**Source:** [monowiki-core/src/bibliography.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L35)

```rust
pub fn collect(&mut self, paths: &[PathBuf]) -> Bibliography
```

Build a merged bibliography for the provided list of paths.

Later files win on key conflicts.

## Reference source: [monowiki-core/src/bibliography.rs L35–L51](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/bibliography.rs#L35)

```rust
    /// Build a merged bibliography for the provided list of paths.
    ///
    /// Later files win on key conflicts.
    pub fn collect(&mut self, paths: &[PathBuf]) -> Bibliography {
        self.preload_paths(paths);

        let mut entries: HashMap<String, Entry> = HashMap::new();
        for path in paths {
            if let Some(lib) = self.cache.get(path) {
                for entry in lib.iter() {
                    entries.insert(entry.key().to_string(), entry.clone());
                }
            }
        }

        Bibliography { entries }
    }
```
