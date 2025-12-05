---
title: config::Config::resolve_relative
description: null
summary: Resolve an arbitrary path relative to the config file location
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-resolve-relative
permalink: null
aliases:
- config::Config::resolve_relative
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

# config::Config::resolve_relative

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L151)

```rust
pub fn resolve_relative(&self, path: &Path) -> PathBuf
```

Resolve an arbitrary path relative to the config file location

## Reference source: [monowiki-core/src/config.rs L151–L154](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L151)

```rust
    /// Resolve an arbitrary path relative to the config file location
    pub fn resolve_relative(&self, path: &Path) -> PathBuf {
        self.resolve_path(path)
    }
```
