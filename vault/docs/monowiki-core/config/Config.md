---
title: config::Config
description: null
summary: Main configuration struct matching monowiki.yml schema
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:config
draft: false
updated: null
slug: config-config
permalink: null
aliases:
- config::Config
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

# config::Config

**Kind:** Struct

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L20)

```rust
pub struct Config
```

Main configuration struct matching monowiki.yml schema

## Reference source: [monowiki-core/src/config.rs L20–L60](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L20)

```rust
/// Main configuration struct matching monowiki.yml schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub site: SiteConfig,
    pub paths: PathsConfig,

    #[serde(default)]
    pub orcid: Option<OrcidConfig>,

    #[serde(default)]
    pub server: ServerConfig,

    // NEW FIELDS for Rust version
    #[serde(default = "default_base_url")]
    pub base_url: String,

    #[serde(default)]
    pub ignore_patterns: Vec<String>,

    #[serde(default)]
    pub bibliography: Vec<PathBuf>,

    #[serde(default)]
    pub theme_overrides: Option<PathBuf>,

    #[serde(default = "default_true")]
    pub enable_rss: bool,

    #[serde(default = "default_true")]
    pub enable_sitemap: bool,

    #[serde(default = "default_true")]
    pub enable_backlinks: bool,

    #[serde(default)]
    pub adapters: Vec<AdapterConfig>,

    // Internal: path to config file (for relative path resolution)
    #[serde(skip)]
    config_path: Option<PathBuf>,
}
```
