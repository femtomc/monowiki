//! Askama template definitions.

use askama::Template;

/// A simple note entry for display in lists
#[derive(Debug, Clone)]
pub struct NoteEntry {
    pub url: String,
    pub title: String,
    pub date: Option<String>,
    pub description: Option<String>,
    pub note_type: String,
}

/// A file entry in the directory tree
#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub url: String,
    pub title: String,
    pub note_type: String,
}

/// A directory entry in the directory tree
#[derive(Debug, Clone)]
pub struct DirectoryNode {
    pub name: String,
    pub files: Vec<FileNode>,
    pub subdirs: Vec<DirectoryNode>,
}

impl DirectoryNode {
    /// Render this directory node and its children to HTML
    pub fn render_to_html(&self) -> String {
        let mut html = String::new();

        html.push_str("<details class=\"directory-node\" open>\n");
        html.push_str("  <summary class=\"directory-name\">\n");
        html.push_str(&format!(
            "    <span class=\"dir-label\">{}/</span>\n",
            self.name
        ));
        html.push_str(&format!(
            "    <span class=\"file-count\">({} files)</span>\n",
            self.files.len()
        ));
        html.push_str("  </summary>\n");

        // Render subdirectories
        if !self.subdirs.is_empty() {
            html.push_str("  <div class=\"subdirs-container\">\n");
            for subdir in &self.subdirs {
                html.push_str(&subdir.render_to_html());
            }
            html.push_str("  </div>\n");
        }

        // Render files
        if !self.files.is_empty() {
            html.push_str("  <ul class=\"file-list\">\n");
            for file in &self.files {
                html.push_str("    <li class=\"file-item\">\n");
                html.push_str(&format!(
                    "      <a href=\"{}\" class=\"file-link\">{}</a>\n",
                    html_escape(&file.url),
                    html_escape(&file.title)
                ));
                html.push_str(&format!(
                    "      <span class=\"file-type-badge\">{}</span>\n",
                    file.note_type
                ));
                html.push_str("    </li>\n");
            }
            html.push_str("  </ul>\n");
        }

        html.push_str("</details>\n");
        html
    }
}

/// HTML escape function to prevent XSS
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
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

#[derive(Debug, Clone)]
pub struct CommentRender {
    pub id: String,
    pub status: String,
    pub resolved: bool,
    pub resolved_anchor: String,
    pub has_anchor: bool,
    pub author: String,
    pub has_author: bool,
    pub quote: String,
    pub has_quote: bool,
    pub body_html: String,
    pub color_bg: String,
    pub color_border: String,
    // Threading fields
    pub parent_id: String,
    pub has_parent: bool,
    pub thread_root: String,
    pub depth: u8,
    pub is_reply: bool,
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

    // Comments/annotations targeting this note
    pub comments: Vec<CommentRender>,

    // Pre-computed flag for whether any unanchored comments exist
    pub has_unanchored_comments: bool,
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

    // Content list (legacy flat list)
    pub items: Vec<NoteEntry>,
    pub papers: Vec<Paper>,

    // Directory tree view (pre-rendered HTML)
    pub directory_tree_html: Option<String>,

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
