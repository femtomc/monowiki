//! Frontmatter parsing from markdown files.

use crate::models::Frontmatter;
use regex::Regex;
use std::sync::OnceLock;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FrontmatterError {
    #[error("Invalid YAML: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

static FRONTMATTER_REGEX: OnceLock<Regex> = OnceLock::new();

fn frontmatter_regex() -> &'static Regex {
    FRONTMATTER_REGEX.get_or_init(|| Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n(.*)$").unwrap())
}

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

/// Extract just the frontmatter without the body
pub fn extract_frontmatter(content: &str) -> Option<Frontmatter> {
    parse_frontmatter(content).ok().map(|(fm, _)| fm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_frontmatter() {
        let content = r#"---
title: Test Post
description: A test post
date: 2025-01-01
type: essay
---

# Hello World

This is the content."#;

        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, "Test Post");
        assert_eq!(fm.description, Some("A test post".to_string()));
        assert_eq!(fm.date, Some("2025-01-01".to_string()));
        assert_eq!(fm.note_type, Some("essay".to_string()));
        assert!(body.contains("# Hello World"));
        assert!(body.contains("This is the content."));
    }

    #[test]
    fn test_parse_minimal_frontmatter() {
        let content = r#"---
title: Minimal Post
---

Content here."#;

        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, "Minimal Post");
        assert_eq!(fm.description, None);
        assert!(body.contains("Content here"));
    }

    #[test]
    fn test_parse_frontmatter_with_tags() {
        let content = r#"---
title: Tagged Post
tags:
  - rust
  - programming
---

Content."#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.tags, vec!["rust", "programming"]);
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just Content\n\nNo frontmatter here.";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, "");
        assert_eq!(body, content);
    }

    #[test]
    fn test_parse_frontmatter_with_draft() {
        let content = r#"---
title: Draft Post
draft: true
---

Content."#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert!(fm.draft);
    }

    #[test]
    fn test_parse_frontmatter_with_permalink() {
        let content = r#"---
title: Custom Permalink
permalink: /custom/path
---

Content."#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.permalink, Some("/custom/path".to_string()));
    }

    #[test]
    fn test_parse_frontmatter_with_typst_preamble() {
        let content = r#"---
title: Math Note
typst_preamble: |
  #let foo = 1
---

Body."#;

        let (fm, _) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.typst_preamble.as_deref(), Some("#let foo = 1"));
    }

    #[test]
    fn test_invalid_yaml() {
        let content = r#"---
title: Test
invalid yaml: [unclosed
---

Content."#;

        assert!(parse_frontmatter(content).is_err());
    }

    #[test]
    fn test_missing_title() {
        let content = r#"---
description: No title
---

Content."#;

        let result = parse_frontmatter(content);
        assert!(result.is_err());
        match result {
            Err(FrontmatterError::MissingField(field)) => assert_eq!(field, "title"),
            _ => panic!("Expected MissingField error"),
        }
    }
}
