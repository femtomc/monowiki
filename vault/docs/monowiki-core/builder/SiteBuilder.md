---
title: builder::SiteBuilder
description: null
summary: Main site builder
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:builder
draft: false
updated: null
slug: builder-sitebuilder
permalink: null
aliases:
- builder::SiteBuilder
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

# builder::SiteBuilder

**Kind:** Struct

**Source:** [monowiki-core/src/builder.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/builder.rs#L31)

```rust
pub struct SiteBuilder
```

Main site builder

## Reference source: [monowiki-core/src/builder.rs L31–L35](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/builder.rs#L31)

```rust
/// Main site builder
pub struct SiteBuilder {
    config: Config,
    processor: MarkdownProcessor,
}
```
