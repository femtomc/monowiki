use blake3;
///! Section-level search indexing for precise search results
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchEntry {
    pub id: String,  // Unique ID: "{slug}#{section-id}"
    pub url: String, // Page URL with anchor: "/page.html#section"
    /// Stable section identifier: heading slug + hash of normalized text
    #[serde(default)]
    pub section_id: String,
    /// Hash of section text (for change detection)
    #[serde(default)]
    pub section_hash: String,
    pub title: String,         // Page title
    pub section_title: String, // Section heading (empty for top matter)
    pub content: String,       // Plain text content
    pub snippet: String,       // First ~200 chars for preview
    pub tags: Vec<String>,     // Page tags
    #[serde(rename = "type")]
    pub doc_type: String, // essay/thought
}

/// Build a granular search index from note HTML
pub fn build_search_index(
    slug: &str,
    title: &str,
    content_html: &str,
    tags: &[String],
    doc_type: &str,
    base_url: &str,
) -> Vec<SearchEntry> {
    // Extract plain text from HTML
    let plain_text = html_to_text(content_html);

    // Parse into sections based on heading markers in HTML
    let sections = extract_sections_from_html(content_html);

    if sections.is_empty() {
        let section_hash = compute_section_hash(&plain_text);
        let section_id = format!("{}-{}", slug, &section_hash[..8]);
        // Fallback: single entry for whole document
        let snippet = create_snippet(&plain_text, 200);
        return vec![SearchEntry {
            id: slug.to_string(),
            url: format!("{}{}.html", base_url, slug),
            section_id,
            section_hash,
            title: title.to_string(),
            section_title: String::new(),
            content: plain_text,
            snippet,
            tags: tags.to_vec(),
            doc_type: doc_type.to_string(),
        }];
    }

    // Create search entry for each section
    sections
        .into_iter()
        .map(|(heading, heading_id, section_text)| {
            let section_id = if heading_id.is_empty() {
                slug.to_string()
            } else {
                format!("{}#{}", slug, heading_id)
            };

            let url = if heading_id.is_empty() {
                format!("{}{}.html", base_url, slug)
            } else {
                format!("{}{}.html#{}", base_url, slug, heading_id)
            };

            let snippet = create_snippet(&section_text, 200);
            let section_hash = compute_section_hash(&section_text);
            let stable_section_id = format!(
                "{}-{}",
                if heading_id.is_empty() {
                    slug.to_string()
                } else {
                    heading_id.clone()
                },
                &section_hash[..8]
            );

            SearchEntry {
                id: section_id,
                section_id: stable_section_id,
                section_hash,
                url,
                title: title.to_string(),
                section_title: heading,
                content: section_text,
                snippet,
                tags: tags.to_vec(),
                doc_type: doc_type.to_string(),
            }
        })
        .collect()
}

fn html_to_text(html: &str) -> String {
    // Simple HTML tag stripper
    let mut result = String::new();
    let mut in_tag = false;

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            result.push(' '); // Add space where tags were
        } else if ch == '>' {
            in_tag = false;
            result.push(' ');
        } else if !in_tag {
            result.push(ch);
        }
    }

    // Clean up whitespace and decode HTML entities
    result
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_sections_from_html(html: &str) -> Vec<(String, String, String)> {
    let mut sections = Vec::new();
    let mut current_heading = String::new();
    let mut current_id = String::new();
    let mut current_content = String::new();

    // Split by heading tags
    let parts: Vec<&str> = html.split("<h").collect();

    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            // Content before first heading
            let text = html_to_text(part);
            if !text.trim().is_empty() {
                current_content.push_str(&text);
                current_content.push(' ');
            }
            continue;
        }

        // Parse heading level and extract id/text
        // Format: "2 id=\"foo\">Bar</h2>rest of content"
        if let Some(close_pos) = part.find("</h") {
            let heading_part = &part[..close_pos];

            // Extract id
            let id = if let Some(id_start) = heading_part.find("id=\"") {
                let id_begin = id_start + 4;
                if let Some(id_end) = heading_part[id_begin..].find('"') {
                    heading_part[id_begin..id_begin + id_end].to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Extract heading text (after '>')
            let heading_text = if let Some(text_start) = heading_part.find('>') {
                html_to_text(&heading_part[text_start + 1..])
            } else {
                String::new()
            };

            // Save previous section
            if !current_content.trim().is_empty() {
                sections.push((
                    current_heading.clone(),
                    current_id.clone(),
                    current_content.trim().to_string(),
                ));
            }

            // Extract content after heading
            let rest = &part[close_pos..];
            if let Some(tag_end) = rest.find('>') {
                let section_content = html_to_text(&rest[tag_end + 1..]);
                current_heading = heading_text;
                current_id = id;
                current_content = section_content;
            }
        }
    }

    // Add final section
    if !current_content.trim().is_empty() {
        sections.push((
            current_heading,
            current_id,
            current_content.trim().to_string(),
        ));
    }

    sections
}

fn create_snippet(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }

    // Find last space within limit
    let truncated: String = chars[..max_chars].iter().collect();
    if let Some(last_space) = truncated.rfind(' ') {
        format!("{}...", &truncated[..last_space])
    } else {
        format!("{}...", truncated)
    }
}

fn compute_section_hash(text: &str) -> String {
    let normalized = text.trim();
    let hash = blake3::hash(normalized.as_bytes());
    hash.to_hex().to_string()
}

/// Lightweight digest for section-level change detection
#[derive(Debug, Clone)]
pub struct SectionDigest {
    pub section_id: String,
    pub heading: String,
    pub hash: String,
    /// Original anchor id (heading slug) if available
    pub anchor_id: Option<String>,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_text() {
        let html = "<p>Hello <strong>world</strong>!</p><p>Second paragraph.</p>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("<"));
    }

    #[test]
    fn test_create_snippet() {
        let text = "This is a long piece of text that should be truncated at a word boundary";
        let snippet = create_snippet(text, 30);
        assert!(snippet.len() <= 33); // 30 + "..."
        assert!(snippet.ends_with("..."));
    }

    #[test]
    fn test_extract_sections() {
        let html = r#"<p>Intro text</p><h2 id="section-1">Section One</h2><p>Content one</p><h2 id="section-2">Section Two</h2><p>Content two</p>"#;
        let sections = extract_sections_from_html(html);
        assert!(sections.len() >= 2);
    }
}
