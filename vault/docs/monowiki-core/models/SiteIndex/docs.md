---
title: models::SiteIndex::docs
description: null
summary: Get all docs (non-draft, type=doc)
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::SiteIndex
draft: false
updated: null
slug: models-siteindex-docs
permalink: null
aliases:
- models::SiteIndex::docs
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

# models::SiteIndex::docs

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L274)

```rust
pub fn docs(&self) -> Vec<&Note>
```

Get all docs (non-draft, type=doc)

## Reference source: [monowiki-core/src/models.rs L274–L280](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L274)

```rust
    /// Get all docs (non-draft, type=doc)
    pub fn docs(&self) -> Vec<&Note> {
        self.notes
            .iter()
            .filter(|n| !n.is_draft() && n.note_type == NoteType::Doc)
            .collect()
    }
```
