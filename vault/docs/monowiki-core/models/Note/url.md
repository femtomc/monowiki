---
title: models::Note::url
description: null
summary: Get the URL path for this note
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::Note
draft: false
updated: null
slug: models-note-url
permalink: null
aliases:
- models::Note::url
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

# models::Note::url

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L154)

```rust
pub fn url(&self) -> String
```

Get the URL path for this note

## Reference source: [monowiki-core/src/models.rs L154–L157](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L154)

```rust
    /// Get the URL path for this note
    pub fn url(&self) -> String {
        format!("/{}", self.output_rel_path())
    }
```
