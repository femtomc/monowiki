---
title: frontmatter::extract_frontmatter
description: null
summary: Extract just the frontmatter without the body
date: null
type: doc
tags:
- rust
- api
- kind:function
- module:frontmatter
draft: false
updated: null
slug: frontmatter-extract-frontmatter
permalink: null
aliases:
- frontmatter::extract_frontmatter
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

# frontmatter::extract_frontmatter

**Kind:** Function

**Source:** [monowiki-core/src/frontmatter.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/frontmatter.rs#L71)

```rust
pub fn extract_frontmatter(content: &str) -> Option<Frontmatter>
```

Extract just the frontmatter without the body

## Reference source: [monowiki-core/src/frontmatter.rs L71–L74](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/frontmatter.rs#L71)

```rust
/// Extract just the frontmatter without the body
pub fn extract_frontmatter(content: &str) -> Option<Frontmatter> {
    parse_frontmatter(content).ok().map(|(fm, _)| fm)
}
```
