---
title: slug::slugify
description: null
summary: Convert a string to a URL-safe slug
date: null
type: doc
tags:
- rust
- api
- kind:function
- module:slug
draft: false
updated: null
slug: slug-slugify
permalink: null
aliases:
- slug::slugify
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

# slug::slugify

**Kind:** Function

**Source:** [monowiki-core/src/slug.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/slug.rs#L6)

```rust
pub fn slugify(input: &str) -> String
```

Convert a string to a URL-safe slug

Rules:
- Lowercase
- Replace whitespace with hyphens
- Remove special characters (except hyphens)
- Collapse multiple hyphens
- Trim leading/trailing hyphens

# Examples

```
use monowiki_core::slugify;

assert_eq!(slugify("Hello World"), "hello-world");
assert_eq!(slugify("Rust & Safety"), "rust-safety");
assert_eq!(slugify("C++ Programming"), "c-programming");
```

## Reference source: [monowiki-core/src/slug.rs L6–L59](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/slug.rs#L6)

```rust
/// Convert a string to a URL-safe slug
///
/// Rules:
/// - Lowercase
/// - Replace whitespace with hyphens
/// - Remove special characters (except hyphens)
/// - Collapse multiple hyphens
/// - Trim leading/trailing hyphens
///
/// # Examples
///
/// ```
/// use monowiki_core::slugify;
///
/// assert_eq!(slugify("Hello World"), "hello-world");
/// assert_eq!(slugify("Rust & Safety"), "rust-safety");
/// assert_eq!(slugify("C++ Programming"), "c-programming");
/// ```
pub fn slugify(input: &str) -> String {
    // Lowercase the input
    let lowercased = input.to_lowercase();

    // Replace whitespace and underscores with hyphens
    let with_hyphens = lowercased
        .graphemes(true)
        .map(|g| match g {
            " " | "_" | "\t" | "\n" => "-",
            _ => g,
        })
        .collect::<String>();

    // Remove characters that aren't alphanumeric, hyphens, or basic latin
    let cleaned = with_hyphens
        .graphemes(true)
        .filter_map(|g| {
            let c = g.chars().next()?;
            if c.is_ascii_alphanumeric() || c == '-' {
                Some(g)
            } else if c.is_alphabetic() {
                // Keep unicode alphabetic characters
                Some(g)
            } else {
                None
            }
        })
        .collect::<String>();

    // Collapse multiple hyphens
    let re = Regex::new(r"-+").unwrap();
    let collapsed = re.replace_all(&cleaned, "-");

    // Trim hyphens from start and end
    collapsed.trim_matches('-').to_string()
}
```
