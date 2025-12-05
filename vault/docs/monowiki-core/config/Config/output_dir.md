---
title: config::Config::output_dir
description: null
summary: Get the output directory, resolved relative to config file
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-output-dir
permalink: null
aliases:
- config::Config::output_dir
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

# config::Config::output_dir

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L146)

```rust
pub fn output_dir(&self) -> PathBuf
```

Get the output directory, resolved relative to config file

## Reference source: [monowiki-core/src/config.rs L146–L149](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L146)

```rust
    /// Get the output directory, resolved relative to config file
    pub fn output_dir(&self) -> PathBuf {
        self.resolve_path(&self.paths.output)
    }
```
