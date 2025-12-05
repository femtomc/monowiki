---
title: search::section_digests_from_html
description: null
summary: Extract section digests (stable IDs + hashes) from rendered HTML
date: null
type: doc
tags:
- rust
- api
- kind:function
- module:search
draft: false
updated: null
slug: search-section-digests-from-html
permalink: null
aliases:
- search::section_digests_from_html
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

# search::section_digests_from_html

**Kind:** Function

**Source:** [monowiki-core/src/search.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/search.rs#L238)

```rust
pub fn section_digests_from_html(slug: &str, title: &str, content_html: &str,) -> Vec<SectionDigest>
```

Extract section digests (stable IDs + hashes) from rendered HTML

## Reference source: [monowiki-core/src/search.rs L238–L254](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/search.rs#L238)

```rust
/// Extract section digests (stable IDs + hashes) from rendered HTML
pub fn section_digests_from_html(
    slug: &str,
    title: &str,
    content_html: &str,
) -> Vec<SectionDigest> {
    let entries = build_search_index(slug, title, content_html, &[], "", "/");
    entries
        .into_iter()
        .map(|entry| SectionDigest {
            section_id: entry.section_id,
            heading: entry.section_title,
            hash: entry.section_hash,
            anchor_id: entry.id.split('#').nth(1).map(|s| s.to_string()),
        })
        .collect()
}
```
