---
title: config::Config::theme_dir
description: null
summary: Get the theme directory (None means use built-in)
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-theme-dir
permalink: null
aliases:
- config::Config::theme_dir
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

# config::Config::theme_dir

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L169)

```rust
pub fn theme_dir(&self) -> Option<PathBuf>
```

Get the theme directory (None means use built-in)

## Reference source: [monowiki-core/src/config.rs L169–L172](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L169)

```rust
    /// Get the theme directory (None means use built-in)
    pub fn theme_dir(&self) -> Option<PathBuf> {
        self.paths.theme.as_ref().map(|p| self.resolve_path(p))
    }
```
