---
title: models::LinkGraph
description: null
summary: Link graph representing connections between notes
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:models
draft: false
updated: null
slug: models-linkgraph
permalink: null
aliases:
- models::LinkGraph
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

# models::LinkGraph

**Kind:** Struct

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L179)

```rust
pub struct LinkGraph
```

Link graph representing connections between notes

## Reference source: [monowiki-core/src/models.rs L179–L187](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L179)

```rust
/// Link graph representing connections between notes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinkGraph {
    /// Map from slug to list of target slugs
    pub outgoing: HashMap<String, Vec<String>>,

    /// Map from slug to list of source slugs (backlinks)
    pub incoming: HashMap<String, Vec<String>>,
}
```
