---
title: config::Config::get
description: null
summary: Get a nested config value using dotted path (e.g., "site.title")
date: null
type: doc
tags:
- rust
- api
- kind:method
- module:config::Config
draft: false
updated: null
slug: config-config-get
permalink: null
aliases:
- config::Config::get
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

# config::Config::get

**Kind:** Method

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L194)

```rust
pub fn get(&self, key: &str) -> Option<String>
```

Get a nested config value using dotted path (e.g., "site.title")

## Reference source: [monowiki-core/src/config.rs L194–L206](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L194)

```rust
    /// Get a nested config value using dotted path (e.g., "site.title")
    pub fn get(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split('.').collect();
        match parts.as_slice() {
            ["site", "title"] => Some(self.site.title.clone()),
            ["site", "author"] => Some(self.site.author.clone()),
            ["site", "description"] => Some(self.site.description.clone()),
            ["site", "url"] => Some(self.site.url.clone()),
            ["site", "intro"] => self.site.intro.clone(),
            ["server", "port"] => Some(self.server.port.to_string()),
            _ => None,
        }
    }
```
