---
title: models::Note::output_rel_path
description: null
summary: Relative output path for this note (no leading slash)
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::Note
draft: false
updated: null
slug: models-note-output-rel-path
permalink: null
aliases:
- models::Note::output_rel_path
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

# models::Note::output_rel_path

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L169)

```rust
pub fn output_rel_path(&self) -> String
```

Relative output path for this note (no leading slash)

## Reference source: [monowiki-core/src/models.rs L169–L176](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L169)

```rust
    /// Relative output path for this note (no leading slash)
    pub fn output_rel_path(&self) -> String {
        if let Some(permalink) = &self.permalink {
            normalize_permalink(permalink)
        } else {
            format!("{}.html", self.slug)
        }
    }
```
