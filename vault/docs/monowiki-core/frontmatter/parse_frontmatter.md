---
title: frontmatter::parse_frontmatter
description: null
summary: Parse frontmatter from markdown content
date: null
type: doc
tags:
- rust
- api
- kind:function
- module:frontmatter
draft: false
updated: null
slug: frontmatter-parse-frontmatter
permalink: null
aliases:
- frontmatter::parse_frontmatter
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

# frontmatter::parse_frontmatter

**Kind:** Function

**Source:** [monowiki-core/src/frontmatter.rs](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/frontmatter.rs#L23)

```rust
pub fn parse_frontmatter(content: &str) -> Result<(Frontmatter, String), FrontmatterError>
```

Parse frontmatter from markdown content

Returns a tuple of (frontmatter, markdown_body).
If no frontmatter is present, returns default frontmatter with the full content as body.

# Example

```
use monowiki_core::frontmatter::parse_frontmatter;

let content = "---\ntitle: My Post\ndate: 2025-01-01\n---\n# Hello World\n";

let (fm, body) = parse_frontmatter(content).unwrap();
assert_eq!(fm.title, "My Post");
assert_eq!(fm.date, Some("2025-01-01".to_string()));
assert!(body.trim().starts_with("# Hello World"));
```

## Reference source: [monowiki-core/src/frontmatter.rs L23–L69](https://github.com/femtomc/monowiki/blob/main/monowiki-core/src/frontmatter.rs#L23)

```rust
/// Parse frontmatter from markdown content
///
/// Returns a tuple of (frontmatter, markdown_body).
/// If no frontmatter is present, returns default frontmatter with the full content as body.
///
/// # Example
///
/// ```
/// use monowiki_core::frontmatter::parse_frontmatter;
///
/// let content = "---\ntitle: My Post\ndate: 2025-01-01\n---\n# Hello World\n";
///
/// let (fm, body) = parse_frontmatter(content).unwrap();
/// assert_eq!(fm.title, "My Post");
/// assert_eq!(fm.date, Some("2025-01-01".to_string()));
/// assert!(body.trim().starts_with("# Hello World"));
/// ```
pub fn parse_frontmatter(content: &str) -> Result<(Frontmatter, String), FrontmatterError> {
    let re = frontmatter_regex();

    if let Some(captures) = re.captures(content) {
        let yaml = captures.get(1).unwrap().as_str();
        let body = captures.get(2).unwrap().as_str();

        let frontmatter: Frontmatter = match serde_yaml::from_str(yaml) {
            Ok(fm) => fm,
            Err(e) => {
                // Check if it's a missing field error
                let err_msg = e.to_string();
                if err_msg.contains("missing field `title`") {
                    return Err(FrontmatterError::MissingField("title".to_string()));
                }
                return Err(FrontmatterError::YamlError(e));
            }
        };

        // Validate required fields
        if frontmatter.title.is_empty() {
            return Err(FrontmatterError::MissingField("title".to_string()));
        }

        Ok((frontmatter, body.to_string()))
    } else {
        // No frontmatter, return default with full content as body
        Ok((Frontmatter::default(), content.to_string()))
    }
}
```
