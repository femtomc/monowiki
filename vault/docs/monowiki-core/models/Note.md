---
title: models::Note
description: null
summary: A single note/post in the site
date: null
type: doc
tags:
- rust
- api
- kind:struct
- module:models
draft: false
updated: null
slug: models-note
permalink: null
aliases:
- models::Note
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

# models::Note

**Kind:** Struct

**Source:** [monowiki-core/src/models.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L103)

```rust
pub struct Note
```

A single note/post in the site

## Reference source: [monowiki-core/src/models.rs L103–L151](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/models.rs#L103)

```rust
/// A single note/post in the site
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// URL slug (e.g., "rust-safety")
    pub slug: String,

    /// Display title
    pub title: String,

    /// Rendered HTML content
    pub content_html: String,

    /// Original frontmatter
    pub frontmatter: Frontmatter,

    /// Note type (essay, thought, etc.)
    pub note_type: NoteType,

    /// Tags for categorization
    pub tags: Vec<String>,

    /// Publication date
    pub date: Option<NaiveDate>,

    /// Last updated date
    pub updated: Option<NaiveDate>,

    /// Alternative slugs/names
    pub aliases: Vec<String>,

    /// Custom permalink (overrides default)
    pub permalink: Option<String>,

    /// Slugs of notes this note links to
    pub outgoing_links: Vec<String>,

    /// Preview text (for link previews)
    pub preview: Option<String>,

    /// Table of contents HTML
    pub toc_html: Option<String>,

    /// Raw markdown body (without frontmatter) for copy/export features
    pub raw_body: Option<String>,

    /// Source path relative to vault root (e.g., "essays/foo.md")
    #[serde(default)]
    pub source_path: Option<String>,
}
```
