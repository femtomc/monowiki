---
title: models::SiteIndex::find_by_slug
description: null
summary: Find a note by slug
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::SiteIndex
draft: false
updated: null
slug: models-siteindex-find-by-slug
permalink: null
aliases:
- models::SiteIndex::find_by_slug
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

# models::SiteIndex::find_by_slug

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L239)

```rust
pub fn find_by_slug(&self, slug: &str) -> Option<&Note>
```

Find a note by slug

## Reference source: [monowiki-core/src/models.rs L239–L242](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L239)

```rust
    /// Find a note by slug
    pub fn find_by_slug(&self, slug: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.slug == slug)
    }
```
