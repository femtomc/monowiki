---
title: models::SiteIndex::thoughts
description: null
summary: Get all thoughts (non-draft, type=thought)
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::SiteIndex
draft: false
updated: null
slug: models-siteindex-thoughts
permalink: null
aliases:
- models::SiteIndex::thoughts
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

# models::SiteIndex::thoughts

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L266)

```rust
pub fn thoughts(&self) -> Vec<&Note>
```

Get all thoughts (non-draft, type=thought)

## Reference source: [monowiki-core/src/models.rs L266–L272](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L266)

```rust
    /// Get all thoughts (non-draft, type=thought)
    pub fn thoughts(&self) -> Vec<&Note> {
        self.notes
            .iter()
            .filter(|n| !n.is_draft() && n.note_type == NoteType::Thought)
            .collect()
    }
```
