---
title: models::SiteIndex::find_by_permalink
description: null
summary: Find a note by permalink
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::SiteIndex
draft: false
updated: null
slug: models-siteindex-find-by-permalink
permalink: null
aliases:
- models::SiteIndex::find_by_permalink
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

# models::SiteIndex::find_by_permalink

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L244)

```rust
pub fn find_by_permalink(&self, permalink: &str) -> Option<&Note>
```

Find a note by permalink

## Reference source: [monowiki-core/src/models.rs L244–L249](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L244)

```rust
    /// Find a note by permalink
    pub fn find_by_permalink(&self, permalink: &str) -> Option<&Note> {
        self.notes
            .iter()
            .find(|n| n.permalink.as_ref().map(|p| p.as_str()) == Some(permalink))
    }
```
