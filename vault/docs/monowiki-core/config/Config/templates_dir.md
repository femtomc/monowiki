---
title: config::Config::templates_dir
description: null
summary: Get the templates directory (None means use built-in)
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-templates-dir
permalink: null
aliases:
- config::Config::templates_dir
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

# config::Config::templates_dir

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L164)

```rust
pub fn templates_dir(&self) -> Option<PathBuf>
```

Get the templates directory (None means use built-in)

## Reference source: [monowiki-core/src/config.rs L164–L167](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L164)

```rust
    /// Get the templates directory (None means use built-in)
    pub fn templates_dir(&self) -> Option<PathBuf> {
        self.paths.templates.as_ref().map(|p| self.resolve_path(p))
    }
```
