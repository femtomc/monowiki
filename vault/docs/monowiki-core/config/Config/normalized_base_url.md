---
title: config::Config::normalized_base_url
description: null
summary: Normalized base URL with leading and trailing slash ("/foo/" or "/")
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-normalized-base-url
permalink: null
aliases:
- config::Config::normalized_base_url
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

# config::Config::normalized_base_url

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L208)

```rust
pub fn normalized_base_url(&self) -> String
```

Normalized base URL with leading and trailing slash ("/foo/" or "/")

## Reference source: [monowiki-core/src/config.rs L208–L211](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L208)

```rust
    /// Normalized base URL with leading and trailing slash ("/foo/" or "/")
    pub fn normalized_base_url(&self) -> String {
        normalize_base_url(&self.base_url)
    }
```
