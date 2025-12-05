---
title: models::SiteIndex::find_by_alias
description: null
summary: Find a note by alias
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:models::SiteIndex
draft: false
updated: null
slug: models-siteindex-find-by-alias
permalink: null
aliases:
- models::SiteIndex::find_by_alias
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

# models::SiteIndex::find_by_alias

**Kind:** Method

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L251)

```rust
pub fn find_by_alias(&self, alias: &str) -> Option<&Note>
```

Find a note by alias

## Reference source: [monowiki-core/src/models.rs L251–L256](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L251)

```rust
    /// Find a note by alias
    pub fn find_by_alias(&self, alias: &str) -> Option<&Note> {
        self.notes
            .iter()
            .find(|n| n.aliases.contains(&alias.to_string()))
    }
```
