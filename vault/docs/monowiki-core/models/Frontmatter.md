---
title: models::Frontmatter
description: null
summary: Frontmatter metadata from markdown files
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:models
draft: false
updated: null
slug: models-frontmatter
permalink: null
aliases:
- models::Frontmatter
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

# models::Frontmatter

**Kind:** Struct

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L41)

```rust
pub struct Frontmatter
```

Frontmatter metadata from markdown files

## Reference source: [monowiki-core/src/models.rs L41–L101](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L41)

```rust
/// Frontmatter metadata from markdown files
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    pub title: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub summary: Option<String>,

    #[serde(default)]
    pub date: Option<String>,

    #[serde(rename = "type")]
    #[serde(default)]
    pub note_type: Option<String>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub draft: bool,

    #[serde(default)]
    pub updated: Option<String>,

    #[serde(default)]
    pub slug: Option<String>,

    #[serde(default)]
    pub permalink: Option<String>,

    #[serde(default)]
    pub aliases: Vec<String>,

    #[serde(default)]
    pub bibliography: Vec<String>,

    #[serde(default)]
    pub target_slug: Option<String>,

    #[serde(default)]
    pub target_anchor: Option<String>,

    #[serde(default)]
    pub git_ref: Option<String>,

    #[serde(default)]
    pub quote: Option<String>,

    #[serde(default)]
    pub author: Option<String>,

    #[serde(default)]
    pub status: Option<String>,

    /// Parent comment id (for replies to comments)
    #[serde(default)]
    pub parent_id: Option<String>,
}
```
