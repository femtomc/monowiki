//! Content model structs for notes, links, and site index.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Type of note content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteType {
    Essay,
    Thought,
    Draft,
    Doc, // For code documentation
}

impl NoteType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "essay" => Some(NoteType::Essay),
            "thought" => Some(NoteType::Thought),
            "draft" => Some(NoteType::Draft),
            "doc" => Some(NoteType::Doc),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            NoteType::Essay => "essay",
            NoteType::Thought => "thought",
            NoteType::Draft => "draft",
            NoteType::Doc => "doc",
        }
    }
}

/// Frontmatter metadata from markdown files
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    pub title: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub summary: Option<String>,

    #[serde(default)]
    pub date: Option<String>,

    #[serde(rename = "type")]
    #[serde(default)]
    pub note_type: Option<String>,

    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default)]
    pub draft: bool,

    #[serde(default)]
    pub updated: Option<String>,

    #[serde(default)]
    pub slug: Option<String>,

    #[serde(default)]
    pub permalink: Option<String>,

    #[serde(default)]
    pub aliases: Vec<String>,
}

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
}

impl Note {
    /// Get the URL path for this note
    pub fn url(&self) -> String {
        format!("/{}", self.output_rel_path())
    }

    /// Get the URL for this note including a base path
    pub fn url_with_base(&self, base_url: &str) -> String {
        format!("{}{}", normalize_base_url(base_url), self.output_rel_path())
    }

    /// Check if this note is a draft
    pub fn is_draft(&self) -> bool {
        self.note_type == NoteType::Draft || self.frontmatter.draft
    }

    /// Relative output path for this note (no leading slash)
    pub fn output_rel_path(&self) -> String {
        if let Some(permalink) = &self.permalink {
            normalize_permalink(permalink)
        } else {
            format!("{}.html", self.slug)
        }
    }
}

/// Link graph representing connections between notes
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LinkGraph {
    /// Map from slug to list of target slugs
    pub outgoing: HashMap<String, Vec<String>>,

    /// Map from slug to list of source slugs (backlinks)
    pub incoming: HashMap<String, Vec<String>>,
}

impl LinkGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a link from source to target
    pub fn add_link(&mut self, source: &str, target: &str) {
        self.outgoing
            .entry(source.to_string())
            .or_default()
            .push(target.to_string());

        self.incoming
            .entry(target.to_string())
            .or_default()
            .push(source.to_string());
    }

    /// Get backlinks for a given note slug
    pub fn backlinks(&self, slug: &str) -> Vec<String> {
        self.incoming.get(slug).cloned().unwrap_or_default()
    }

    /// Get outgoing links for a given note slug
    pub fn outgoing(&self, slug: &str) -> Vec<String> {
        self.outgoing.get(slug).cloned().unwrap_or_default()
    }
}

/// Complete site index containing all notes and the link graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteIndex {
    pub notes: Vec<Note>,
    pub graph: LinkGraph,
}

impl SiteIndex {
    pub fn new() -> Self {
        Self {
            notes: Vec::new(),
            graph: LinkGraph::new(),
        }
    }

    /// Find a note by slug
    pub fn find_by_slug(&self, slug: &str) -> Option<&Note> {
        self.notes.iter().find(|n| n.slug == slug)
    }

    /// Find a note by permalink
    pub fn find_by_permalink(&self, permalink: &str) -> Option<&Note> {
        self.notes
            .iter()
            .find(|n| n.permalink.as_ref().map(|p| p.as_str()) == Some(permalink))
    }

    /// Find a note by alias
    pub fn find_by_alias(&self, alias: &str) -> Option<&Note> {
        self.notes
            .iter()
            .find(|n| n.aliases.contains(&alias.to_string()))
    }

    /// Get all essays (non-draft, type=essay)
    pub fn essays(&self) -> Vec<&Note> {
        self.notes
            .iter()
            .filter(|n| !n.is_draft() && n.note_type == NoteType::Essay)
            .collect()
    }

    /// Get all thoughts (non-draft, type=thought)
    pub fn thoughts(&self) -> Vec<&Note> {
        self.notes
            .iter()
            .filter(|n| !n.is_draft() && n.note_type == NoteType::Thought)
            .collect()
    }
}

impl Default for SiteIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_type_conversion() {
        assert_eq!(NoteType::from_str("essay"), Some(NoteType::Essay));
        assert_eq!(NoteType::from_str("THOUGHT"), Some(NoteType::Thought));
        assert_eq!(NoteType::from_str("Draft"), Some(NoteType::Draft));
        assert_eq!(NoteType::from_str("invalid"), None);
    }

    #[test]
    fn test_link_graph() {
        let mut graph = LinkGraph::new();
        graph.add_link("rust-safety", "memory-model");
        graph.add_link("rust-safety", "ownership");
        graph.add_link("ownership", "memory-model");

        assert_eq!(graph.outgoing("rust-safety").len(), 2);
        assert_eq!(graph.backlinks("memory-model").len(), 2);
        assert!(graph
            .backlinks("memory-model")
            .contains(&"rust-safety".to_string()));
        assert!(graph
            .backlinks("memory-model")
            .contains(&"ownership".to_string()));
    }

    #[test]
    fn test_note_url() {
        let note_default = Note {
            slug: "test-note".into(),
            title: "Test".into(),
            content_html: "".into(),
            frontmatter: Frontmatter::default(),
            note_type: NoteType::Essay,
            tags: vec![],
            date: None,
            updated: None,
            aliases: vec![],
            permalink: None,
            outgoing_links: vec![],
            preview: None,
            toc_html: None,
            raw_body: None,
        };

        assert_eq!(note_default.url(), "/test-note.html");

        let note_permalink = Note {
            permalink: Some("/custom/path".into()),
            ..note_default
        };

        assert_eq!(note_permalink.url(), "/custom/path.html");
        assert_eq!(note_permalink.output_rel_path(), "custom/path.html");
        assert_eq!(
            note_permalink.url_with_base("/blog"),
            "/blog/custom/path.html"
        );
    }
}

fn normalize_permalink(permalink: &str) -> String {
    let mut p = permalink.trim().trim_start_matches('/').to_string();

    if p.ends_with('/') {
        p = format!("{}index.html", p.trim_end_matches('/'));
    } else if !p.ends_with(".html") {
        p = format!("{}.html", p);
    }

    p
}

fn normalize_base_url(base_url: &str) -> String {
    crate::config::normalize_base_url(base_url)
}
