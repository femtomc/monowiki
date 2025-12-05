---
title: config::Config::vault_dir
description: null
summary: Get the vault directory, resolved relative to config file
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-vault-dir
permalink: null
aliases:
- config::Config::vault_dir
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

# config::Config::vault_dir

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L141)

```rust
pub fn vault_dir(&self) -> PathBuf
```

Get the vault directory, resolved relative to config file

## Reference source: [monowiki-core/src/config.rs L141–L144](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L141)

```rust
    /// Get the vault directory, resolved relative to config file
    pub fn vault_dir(&self) -> PathBuf {
        self.resolve_path(&self.paths.vault)
    }
```
