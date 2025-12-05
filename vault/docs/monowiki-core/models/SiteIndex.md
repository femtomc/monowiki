---
title: models::SiteIndex
description: null
summary: Complete site index containing all notes and the link graph
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:models
draft: false
updated: null
slug: models-siteindex
permalink: null
aliases:
- models::SiteIndex
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

# models::SiteIndex

**Kind:** Struct

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L218)

```rust
pub struct SiteIndex
```

Complete site index containing all notes and the link graph

## Reference source: [monowiki-core/src/models.rs L218–L227](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L218)

```rust
/// Complete site index containing all notes and the link graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteIndex {
    pub notes: Vec<Note>,
    pub graph: LinkGraph,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub comments: Vec<Comment>,
}
```
