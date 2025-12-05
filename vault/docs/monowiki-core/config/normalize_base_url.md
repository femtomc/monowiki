---
title: config::normalize_base_url
description: null
summary: Ensure base URLs have a leading and trailing slash
date: null
type: doc
tags:
- rust
- api
- kind:function
- module:config
draft: false
updated: null
slug: config-normalize-base-url
permalink: null
aliases:
- config::normalize_base_url
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

# config::normalize_base_url

**Kind:** Function

**Source:** [monowiki-core/src/config.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L222)

```rust
pub fn normalize_base_url(raw: &str) -> String
```

Ensure base URLs have a leading and trailing slash

## Reference source: [monowiki-core/src/config.rs L222–L249](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/config.rs#L222)

```rust
/// Ensure base URLs have a leading and trailing slash
pub fn normalize_base_url(raw: &str) -> String {
    if raw.is_empty() {
        return "/".to_string();
    }

    let mut s = raw.trim().to_string();
    if !s.starts_with('/') {
        s.insert(0, '/');
    }
    if !s.ends_with('/') {
        s.push('/');
    }

    // Collapse duplicate slashes (but keep leading)
    while s.contains("//") {
        s = s.replace("//", "/");
        if !s.starts_with('/') {
            s.insert(0, '/');
        }
    }

    if s.is_empty() {
        "/".to_string()
    } else {
        s
    }
}
```
