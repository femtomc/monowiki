---
title: models::Note::url_with_base
description: null
summary: Get the URL for this note including a base path
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::Note
draft: false
updated: null
slug: models-note-url-with-base
permalink: null
aliases:
- models::Note::url_with_base
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

# models::Note::url_with_base

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L159)

```rust
pub fn url_with_base(&self, base_url: &str) -> String
```

Get the URL for this note including a base path

## Reference source: [monowiki-core/src/models.rs L159–L162](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L159)

```rust
    /// Get the URL for this note including a base path
    pub fn url_with_base(&self, base_url: &str) -> String {
        format!("{}{}", normalize_base_url(base_url), self.output_rel_path())
    }
```
