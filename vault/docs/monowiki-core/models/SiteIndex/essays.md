---
title: models::SiteIndex::essays
description: null
summary: Get all essays (non-draft, type=essay)
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::SiteIndex
draft: false
updated: null
slug: models-siteindex-essays
permalink: null
aliases:
- models::SiteIndex::essays
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

# models::SiteIndex::essays

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L258)

```rust
pub fn essays(&self) -> Vec<&Note>
```

Get all essays (non-draft, type=essay)

## Reference source: [monowiki-core/src/models.rs L258–L264](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L258)

```rust
    /// Get all essays (non-draft, type=essay)
    pub fn essays(&self) -> Vec<&Note> {
        self.notes
            .iter()
            .filter(|n| !n.is_draft() && n.note_type == NoteType::Essay)
            .collect()
    }
```
