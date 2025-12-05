---
title: models::LinkGraph::backlinks
description: null
summary: Get backlinks for a given note slug
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::LinkGraph
draft: false
updated: null
slug: models-linkgraph-backlinks
permalink: null
aliases:
- models::LinkGraph::backlinks
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

# models::LinkGraph::backlinks

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L207)

```rust
pub fn backlinks(&self, slug: &str) -> Vec<String>
```

Get backlinks for a given note slug

## Reference source: [monowiki-core/src/models.rs L207–L210](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L207)

```rust
    /// Get backlinks for a given note slug
    pub fn backlinks(&self, slug: &str) -> Vec<String> {
        self.incoming.get(slug).cloned().unwrap_or_default()
    }
```
