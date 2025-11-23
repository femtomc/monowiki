//! Slug generation and normalization.

use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

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

/// Normalize a slug (ensure it's properly formatted)
pub fn normalize_slug(slug: &str) -> String {
    slugify(slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("Rust Programming"), "rust-programming");
    }

    #[test]
    fn test_special_characters() {
        assert_eq!(slugify("Rust & Safety"), "rust-safety");
        assert_eq!(slugify("C++ Programming"), "c-programming");
        assert_eq!(slugify("Node.js Tips"), "nodejs-tips");
        assert_eq!(slugify("What's new?"), "whats-new");
    }

    #[test]
    fn test_unicode() {
        assert_eq!(slugify("Café"), "café");
        assert_eq!(slugify("naïve"), "naïve");
    }

    #[test]
    fn test_multiple_spaces() {
        assert_eq!(slugify("Hello    World"), "hello-world");
        assert_eq!(slugify("Multiple   Spaces   Here"), "multiple-spaces-here");
    }

    #[test]
    fn test_leading_trailing_hyphens() {
        assert_eq!(slugify("  Hello World  "), "hello-world");
        assert_eq!(slugify("-Leading Hyphen"), "leading-hyphen");
        assert_eq!(slugify("Trailing Hyphen-"), "trailing-hyphen");
    }

    #[test]
    fn test_underscores() {
        assert_eq!(slugify("hello_world"), "hello-world");
        assert_eq!(slugify("rust_lang_basics"), "rust-lang-basics");
    }

    #[test]
    fn test_mixed_case() {
        assert_eq!(slugify("CamelCase"), "camelcase");
        assert_eq!(slugify("PascalCase"), "pascalcase");
        assert_eq!(slugify("UPPERCASE"), "uppercase");
    }

    #[test]
    fn test_empty_and_special_only() {
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("!!!"), "");
        assert_eq!(slugify("   "), "");
    }

    #[test]
    fn test_normalize_slug() {
        assert_eq!(normalize_slug("Already-Good"), "already-good");
        assert_eq!(normalize_slug("Needs_Fixing"), "needs-fixing");
    }
}
