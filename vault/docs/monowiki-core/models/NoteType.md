---
title: models::NoteType
description: null
summary: Type of note content
date: null
type: doc
tags:
- rust
- api
- kind:enum
- module:models
draft: false
updated: null
slug: models-notetype
permalink: null
aliases:
- models::NoteType
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

# models::NoteType

**Kind:** Enum

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L7)

```rust
pub enum NoteType
```

Type of note content

## Reference source: [monowiki-core/src/models.rs L7–L16](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L7)

```rust
/// Type of note content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteType {
    Essay,
    Thought,
    Draft,
    Doc, // For code documentation
    Comment,
}
```
