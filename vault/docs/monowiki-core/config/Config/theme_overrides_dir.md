---
title: config::Config::theme_overrides_dir
description: null
summary: Get theme overrides directory (copied after the main theme)
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-theme-overrides-dir
permalink: null
aliases:
- config::Config::theme_overrides_dir
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

# config::Config::theme_overrides_dir

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L174)

```rust
pub fn theme_overrides_dir(&self) -> Option<PathBuf>
```

Get theme overrides directory (copied after the main theme)

## Reference source: [monowiki-core/src/config.rs L174–L177](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L174)

```rust
    /// Get theme overrides directory (copied after the main theme)
    pub fn theme_overrides_dir(&self) -> Option<PathBuf> {
        self.theme_overrides.as_ref().map(|p| self.resolve_path(p))
    }
```
