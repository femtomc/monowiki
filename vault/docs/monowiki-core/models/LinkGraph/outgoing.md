---
title: models::LinkGraph::outgoing
description: null
summary: Get outgoing links for a given note slug
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::LinkGraph
draft: false
updated: null
slug: models-linkgraph-outgoing
permalink: null
aliases:
- models::LinkGraph::outgoing
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

# models::LinkGraph::outgoing

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L212)

```rust
pub fn outgoing(&self, slug: &str) -> Vec<String>
```

Get outgoing links for a given note slug

## Reference source: [monowiki-core/src/models.rs L212–L215](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L212)

```rust
    /// Get outgoing links for a given note slug
    pub fn outgoing(&self, slug: &str) -> Vec<String> {
        self.outgoing.get(slug).cloned().unwrap_or_default()
    }
```
