---
title: config::Config::bibliography_paths
description: null
summary: Get bibliography files, resolved relative to config file
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-bibliography-paths
permalink: null
aliases:
- config::Config::bibliography_paths
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

# config::Config::bibliography_paths

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L156)

```rust
pub fn bibliography_paths(&self) -> Vec<PathBuf>
```

Get bibliography files, resolved relative to config file

## Reference source: [monowiki-core/src/config.rs L156–L162](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L156)

```rust
    /// Get bibliography files, resolved relative to config file
    pub fn bibliography_paths(&self) -> Vec<PathBuf> {
        self.bibliography
            .iter()
            .map(|p| self.resolve_path(p))
            .collect()
    }
```
