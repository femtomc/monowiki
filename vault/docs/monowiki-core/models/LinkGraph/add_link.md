---
title: models::LinkGraph::add_link
description: null
summary: Add a link from source to target
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::LinkGraph
draft: false
updated: null
slug: models-linkgraph-add-link
permalink: null
aliases:
- models::LinkGraph::add_link
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

# models::LinkGraph::add_link

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L194)

```rust
pub fn add_link(&mut self, source: &str, target: &str)
```

Add a link from source to target

## Reference source: [monowiki-core/src/models.rs L194–L205](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L194)

```rust
    /// Add a link from source to target
    pub fn add_link(&mut self, source: &str, target: &str) {
        self.outgoing
            .entry(source.to_string())
            .or_default()
            .push(target.to_string());

        self.incoming
            .entry(target.to_string())
            .or_default()
            .push(source.to_string());
    }
```
