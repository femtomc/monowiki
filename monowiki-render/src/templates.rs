//! Askama template definitions.

use askama::Template;

/// A simple note entry for display in lists
#[derive(Debug, Clone)]
pub struct NoteEntry {
    pub url: String,
    pub title: String,
    pub date: Option<String>,
    pub description: Option<String>,
}

/// A paper from ORCID
#[derive(Debug, Clone)]
pub struct Paper {
    pub title: String,
    pub url: Option<String>,
    pub year: Option<u32>,
    pub authors: Vec<Author>,
    pub journal: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Author {
    pub name: String,
    pub is_me: bool,
}

/// A backlink entry
#[derive(Debug, Clone)]
pub struct BacklinkEntry {
    pub url: String,
    pub title: String,
}

/// Post/note page template
#[derive(Template)]
#[template(path = "post.html")]
pub struct PostTemplate {
    // Page metadata
    pub title: String,
    pub description: String,
    pub date: Option<String>,
    pub updated: Option<String>,
    pub tags: Vec<String>,

    // Content
    pub content: String,
    pub toc_html: Option<String>,

    // Site metadata
    pub site_title: String,
    pub site_author: String,
    pub year: i32,

    // Navigation
    pub nav_home: String,
    pub nav_about: String,
    pub nav_github: String,
    pub has_about: bool,
    pub has_github: bool,

    // Path adjustments (for nested pages)
    pub css_path: String,

    // Backlinks
    pub backlinks: Vec<BacklinkEntry>,

    // Site base URL and current slug (for frontend scripts)
    pub base_url: String,
    pub slug: String,

    // Raw markdown source (without frontmatter) for copy/export
    pub source: Option<String>,
}

/// Index page template
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    // Site metadata
    pub site_title: String,
    pub site_description: String,
    pub site_author: String,
    pub site_intro: Option<String>,
    pub year: i32,

    // Navigation
    pub nav_home: String,
    pub nav_about: String,
    pub nav_github: String,
    pub has_about: bool,
    pub has_github: bool,

    // Content lists
    pub essays: Vec<NoteEntry>,
    pub thoughts: Vec<NoteEntry>,
    pub papers: Vec<Paper>,

    // Site base URL (for frontend scripts)
    pub base_url: String,
}

/// 404 error page template
#[derive(Template)]
#[template(path = "404.html")]
pub struct NotFoundTemplate {
    // Site metadata
    pub site_title: String,
    pub site_author: String,
    pub year: i32,

    // Navigation
    pub nav_home: String,
    pub nav_about: String,
    pub nav_github: String,
    pub has_about: bool,
    pub has_github: bool,

    // Path adjustments
    pub css_path: String,

    // Site base URL (for frontend scripts)
    pub base_url: String,
}
