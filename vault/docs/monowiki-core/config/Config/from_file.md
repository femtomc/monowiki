---
title: config::Config::from_file
description: null
summary: Load configuration from a YAML file
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-from-file
permalink: null
aliases:
- config::Config::from_file
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

# config::Config::from_file

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L129)

```rust
pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError>
```

Load configuration from a YAML file

## Reference source: [monowiki-core/src/config.rs L129–L139](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L129)

```rust
    /// Load configuration from a YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&contents)?;

        // Store config file path for relative path resolution
        config.config_path = Some(path.to_path_buf());

        Ok(config)
    }
```
