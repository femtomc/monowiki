---
title: models::Note::is_draft
description: null
summary: Check if this note is a draft
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::Note
draft: false
updated: null
slug: models-note-is-draft
permalink: null
aliases:
- models::Note::is_draft
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

# models::Note::is_draft

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L164)

```rust
pub fn is_draft(&self) -> bool
```

Check if this note is a draft

## Reference source: [monowiki-core/src/models.rs L164–L167](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L164)

```rust
    /// Check if this note is a draft
    pub fn is_draft(&self) -> bool {
        self.note_type == NoteType::Draft || self.frontmatter.draft
    }
```
