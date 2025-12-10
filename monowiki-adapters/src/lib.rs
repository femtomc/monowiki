//! # monowiki-adapters
//!
//! Language-agnostic code documentation adapters using tree-sitter.
//!
//! This crate extracts documentation from source code across multiple languages
//! using tree-sitter for parsing. Each language is feature-gated:
//!
//! - `rust` (default) - Rust source files (`.rs`)
//! - `python` - Python source files (`.py`)
//! - `typescript` - TypeScript/JavaScript files (`.ts`, `.tsx`, `.js`, `.jsx`)
//! - `go` - Go source files (`.go`)
//!
//! ## Usage
//!
//! ```ignore
//! use monowiki_adapters::{adapter_by_name, AdapterOptions};
//!
//! let adapter = adapter_by_name("rust").unwrap();
//! let outputs = adapter.extract(
//!     Path::new("src"),
//!     Some("https://github.com/user/repo"),
//!     &AdapterOptions::new(),
//! )?;
//! ```

use monowiki_core::Frontmatter;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[cfg(feature = "rust")]
mod lang_rust;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Failed to read source file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse source: {0}")]
    ParseError(String),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

// ============================================================================
// Core Types
// ============================================================================

/// The kind of documented item (struct, function, method, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocKind {
    Module,
    Function,
    Struct,
    Enum,
    Trait,
    Method,
    Class,
    Interface,
    Type,
    Constant,
}

impl DocKind {
    /// Returns the lowercase string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            DocKind::Module => "module",
            DocKind::Function => "function",
            DocKind::Struct => "struct",
            DocKind::Enum => "enum",
            DocKind::Trait => "trait",
            DocKind::Method => "method",
            DocKind::Class => "class",
            DocKind::Interface => "interface",
            DocKind::Type => "type",
            DocKind::Constant => "constant",
        }
    }

    /// Returns the display name (capitalized)
    pub fn display(&self) -> &'static str {
        match self {
            DocKind::Module => "Module",
            DocKind::Function => "Function",
            DocKind::Struct => "Struct",
            DocKind::Enum => "Enum",
            DocKind::Trait => "Trait",
            DocKind::Method => "Method",
            DocKind::Class => "Class",
            DocKind::Interface => "Interface",
            DocKind::Type => "Type",
            DocKind::Constant => "Constant",
        }
    }
}

/// Metadata about source location for linking back to code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Relative file path from source root
    pub file: PathBuf,
    /// Starting line number (1-indexed)
    pub start_line: Option<u32>,
    /// Ending line number (1-indexed)
    pub end_line: Option<u32>,
    /// Repository URL for generating links
    pub repo_url: Option<String>,
}

impl SourceLocation {
    /// Generate a URL to this source location (e.g., GitHub)
    pub fn to_url(&self) -> Option<String> {
        let repo = self.repo_url.as_ref()?;
        let file = self.file.to_string_lossy().replace('\\', "/");
        let anchor = self.start_line.map(|l| format!("#L{}", l)).unwrap_or_default();
        Some(format!("{}/blob/main/{}{}", repo.trim_end_matches('/'), file, anchor))
    }

    /// Generate a display string for the source location
    pub fn display(&self) -> String {
        let file = self.file.to_string_lossy();
        match (self.start_line, self.end_line) {
            (Some(start), Some(end)) if start != end => format!("{} L{}–L{}", file, start, end),
            (Some(line), _) => format!("{} L{}", file, line),
            _ => file.to_string(),
        }
    }
}

/// A documented item extracted from source code
#[derive(Debug, Clone)]
pub struct DocItem {
    /// Fully qualified name (e.g., "config::Config::from_file")
    pub name: String,
    /// The kind of item
    pub kind: DocKind,
    /// Documentation text (cleaned of comment markers)
    pub docs: Option<String>,
    /// Code signature (e.g., "pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self>")
    pub signature: String,
    /// Full source code of the item
    pub source: Option<String>,
    /// Source location
    pub location: SourceLocation,
    /// Module path components (e.g., ["config"] for config::Config)
    pub module_path: Vec<String>,
    /// Container type for methods (e.g., "Config" for Config::from_file)
    pub container: Option<String>,
}

impl DocItem {
    /// Get the first line of documentation as a summary
    pub fn summary(&self) -> Option<String> {
        self.docs.as_ref().and_then(|d| {
            d.lines()
                .find(|l| !l.trim().is_empty())
                .map(|s| s.trim().to_string())
        })
    }
}

/// Output from an adapter - ready to be written as markdown
#[derive(Debug, Clone)]
pub struct AdapterOutput {
    /// Relative path for the output file (e.g., "config/Config/from_file.md")
    pub output_rel_path: PathBuf,
    /// Frontmatter for the markdown file
    pub frontmatter: Frontmatter,
    /// Markdown body content
    pub body_md: String,
    /// Source location metadata
    pub source: SourceLocation,
    /// Language identifier
    pub language: String,
    /// Kind of item
    pub kind: DocKind,
}

impl AdapterOutput {
    /// Convert to full markdown with YAML frontmatter
    pub fn to_markdown(&self) -> Result<String, AdapterError> {
        let yaml = serde_yaml::to_string(&self.frontmatter)
            .map_err(|e| AdapterError::SerializationError(e.to_string()))?;
        Ok(format!("---\n{}---\n\n{}", yaml, self.body_md))
    }
}

// ============================================================================
// Adapter Options
// ============================================================================

/// Configuration options passed to adapters
#[derive(Debug, Clone, Default)]
pub struct AdapterOptions {
    options: HashMap<String, Value>,
}

impl AdapterOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_map(options: HashMap<String, Value>) -> Self {
        Self { options }
    }

    /// Get a boolean option with a default value
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.options
            .get(key)
            .and_then(|v| v.as_bool())
            .unwrap_or(default)
    }

    /// Get a string option
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.options
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Set a boolean option
    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.options.insert(key.to_string(), Value::Bool(value));
    }

    /// Set a string option
    pub fn set_string(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), Value::String(value.to_string()));
    }
}

// ============================================================================
// Adapter Trait
// ============================================================================

/// Trait for language-specific documentation adapters
pub trait DocAdapter: Send + Sync {
    /// Name of this adapter (e.g., "rust", "python")
    fn name(&self) -> &str;

    /// File extensions this adapter handles (e.g., ["rs"] for Rust)
    fn extensions(&self) -> &[&str];

    /// Extract documentation from all source files in a directory
    fn extract(
        &self,
        source_root: &Path,
        repo_url: Option<&str>,
        options: &AdapterOptions,
    ) -> Result<Vec<AdapterOutput>, AdapterError>;
}

// ============================================================================
// Adapter Registry
// ============================================================================

/// Get an adapter by name (e.g., "rust", "python")
pub fn adapter_by_name(name: &str) -> Option<Box<dyn DocAdapter>> {
    match name {
        #[cfg(feature = "rust")]
        "rust" => Some(Box::new(lang_rust::RustAdapter::new())),
        _ => None,
    }
}

/// List names of all available adapters (based on enabled features)
pub fn available_adapters() -> Vec<&'static str> {
    let mut adapters = Vec::new();

    #[cfg(feature = "rust")]
    adapters.push("rust");

    #[cfg(feature = "python")]
    adapters.push("python");

    #[cfg(feature = "typescript")]
    adapters.push("typescript");

    #[cfg(feature = "go")]
    adapters.push("go");

    adapters
}

// ============================================================================
// Shared Utilities
// ============================================================================

/// Build frontmatter for a documented item
pub fn build_frontmatter(item: &DocItem, language: &str) -> Frontmatter {
    let module_tag = if !item.module_path.is_empty() {
        Some(format!("module:{}", item.module_path.join("::")))
    } else {
        None
    };

    let container_tag = item.container.as_ref().map(|c| {
        let full_path: Vec<&str> = item.module_path.iter().map(|s| s.as_str()).chain(std::iter::once(c.as_str())).collect();
        format!("module:{}", full_path.join("::"))
    });

    let mut tags = vec![
        language.to_string(),
        "api".to_string(),
        format!("kind:{}", item.kind.as_str()),
    ];

    if let Some(mt) = container_tag.or(module_tag) {
        tags.push(mt);
    }

    let slug = monowiki_core::slugify(&item.name.replace("::", "-"));

    // Build parent_item for methods (e.g., "config::Config" for config::Config::from_file)
    let parent_item = item.container.as_ref().map(|container| {
        if item.module_path.is_empty() {
            container.clone()
        } else {
            format!("{}::{}", item.module_path.join("::"), container)
        }
    });

    // Build source line range string
    let source_lines = match (item.location.start_line, item.location.end_line) {
        (Some(start), Some(end)) => Some(format!("{}-{}", start, end)),
        (Some(start), None) => Some(format!("{}", start)),
        _ => None,
    };

    Frontmatter {
        title: item.name.clone(),
        slug: Some(slug),
        note_type: Some("doc".to_string()),
        tags,
        aliases: vec![item.name.clone()],
        summary: item.summary(),
        // API-specific fields
        parent_item,
        doc_kind: Some(item.kind.as_str().to_string()),
        source_url: item.location.to_url(),
        source_file: Some(item.location.file.to_string_lossy().to_string()),
        source_lines,
        signature: Some(item.signature.clone()),
        ..Default::default()
    }
}

/// Build markdown body for a documented item
///
/// The body is now simpler since metadata (kind, source, signature) is in frontmatter.
/// The template can render those fields with custom styling.
/// The body contains: documentation text and optionally the full source.
pub fn build_body(item: &DocItem, language: &str) -> String {
    let mut body = String::new();

    // Documentation text (the human-written content from docstrings)
    // This is the primary content - supports full markdown including wikilinks
    if let Some(ref docs) = item.docs {
        let trimmed = docs.trim();
        if !trimmed.is_empty() {
            body.push_str(trimmed);
            body.push_str("\n\n");
        }
    }

    // Full source code in a collapsible details block
    // This keeps the page clean but source is accessible
    if let Some(ref source) = item.source {
        body.push_str("<details>\n<summary>Source</summary>\n\n");
        body.push_str(&format!("```{}\n{}\n```\n\n", language, source));
        body.push_str("</details>\n");
    }

    body
}

/// Build output path for a documented item
pub fn build_output_path(item: &DocItem) -> PathBuf {
    let mut path = PathBuf::new();

    // Add module path components
    for part in &item.module_path {
        path.push(part);
    }

    // Add container if present (for methods)
    if let Some(ref container) = item.container {
        path.push(container);
    }

    // Add item name as filename
    let file_name = item.name.split("::").last().unwrap_or(&item.name);
    path.push(format!("{}.md", file_name));

    path
}

/// Walk source files matching given extensions
pub fn walk_source_files<'a>(
    root: &'a Path,
    extensions: &'a [&'a str],
) -> impl Iterator<Item = PathBuf> + 'a {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(move |e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| extensions.contains(&ext))
                .unwrap_or(false)
        })
        .map(|e| e.into_path())
}

/// Check if a path appears to be a test file
pub fn is_test_path(path: &Path) -> bool {
    let test_dirs = ["tests", "__tests__", "test", "testing"];
    let has_test_dir = path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| test_dirs.contains(&s))
            .unwrap_or(false)
    });

    let is_test_file = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| {
            s.ends_with("_test")
                || s.ends_with("_tests")
                || s.ends_with(".test")
                || s.starts_with("test_")
        })
        .unwrap_or(false);

    has_test_dir || is_test_file
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_kind_as_str() {
        assert_eq!(DocKind::Function.as_str(), "function");
        assert_eq!(DocKind::Struct.as_str(), "struct");
        assert_eq!(DocKind::Method.as_str(), "method");
    }

    #[test]
    fn test_doc_kind_display() {
        assert_eq!(DocKind::Function.display(), "Function");
        assert_eq!(DocKind::Struct.display(), "Struct");
        assert_eq!(DocKind::Method.display(), "Method");
    }

    #[test]
    fn test_source_location_to_url() {
        let loc = SourceLocation {
            file: PathBuf::from("src/lib.rs"),
            start_line: Some(42),
            end_line: Some(50),
            repo_url: Some("https://github.com/user/repo".to_string()),
        };
        assert_eq!(
            loc.to_url(),
            Some("https://github.com/user/repo/blob/main/src/lib.rs#L42".to_string())
        );
    }

    #[test]
    fn test_source_location_to_url_no_repo() {
        let loc = SourceLocation {
            file: PathBuf::from("src/lib.rs"),
            start_line: Some(42),
            end_line: None,
            repo_url: None,
        };
        assert_eq!(loc.to_url(), None);
    }

    #[test]
    fn test_source_location_display() {
        let loc = SourceLocation {
            file: PathBuf::from("src/lib.rs"),
            start_line: Some(42),
            end_line: Some(50),
            repo_url: None,
        };
        assert_eq!(loc.display(), "src/lib.rs L42–L50");

        let loc2 = SourceLocation {
            file: PathBuf::from("src/lib.rs"),
            start_line: Some(42),
            end_line: Some(42),
            repo_url: None,
        };
        assert_eq!(loc2.display(), "src/lib.rs L42");

        let loc3 = SourceLocation {
            file: PathBuf::from("src/lib.rs"),
            start_line: None,
            end_line: None,
            repo_url: None,
        };
        assert_eq!(loc3.display(), "src/lib.rs");
    }

    #[test]
    fn test_doc_item_summary() {
        let item = DocItem {
            name: "test".to_string(),
            kind: DocKind::Function,
            docs: Some("First line summary.\n\nMore details here.".to_string()),
            signature: "fn test()".to_string(),
            source: None,
            location: SourceLocation {
                file: PathBuf::from("lib.rs"),
                start_line: None,
                end_line: None,
                repo_url: None,
            },
            module_path: vec![],
            container: None,
        };
        assert_eq!(item.summary(), Some("First line summary.".to_string()));
    }

    #[test]
    fn test_doc_item_summary_empty_lines() {
        let item = DocItem {
            name: "test".to_string(),
            kind: DocKind::Function,
            docs: Some("\n\n  Real summary here.\n".to_string()),
            signature: "fn test()".to_string(),
            source: None,
            location: SourceLocation {
                file: PathBuf::from("lib.rs"),
                start_line: None,
                end_line: None,
                repo_url: None,
            },
            module_path: vec![],
            container: None,
        };
        assert_eq!(item.summary(), Some("Real summary here.".to_string()));
    }

    #[test]
    fn test_adapter_options_bool() {
        let mut opts = AdapterOptions::new();
        assert!(!opts.get_bool("include_private", false));
        assert!(opts.get_bool("include_private", true));

        opts.set_bool("include_private", true);
        assert!(opts.get_bool("include_private", false));
    }

    #[test]
    fn test_adapter_options_string() {
        let mut opts = AdapterOptions::new();
        assert_eq!(opts.get_string("prefix"), None);

        opts.set_string("prefix", "my_prefix");
        assert_eq!(opts.get_string("prefix"), Some("my_prefix".to_string()));
    }

    #[test]
    fn test_build_output_path_simple() {
        let item = DocItem {
            name: "slugify".to_string(),
            kind: DocKind::Function,
            docs: None,
            signature: "".to_string(),
            source: None,
            location: SourceLocation {
                file: PathBuf::from("slug.rs"),
                start_line: None,
                end_line: None,
                repo_url: None,
            },
            module_path: vec!["slug".to_string()],
            container: None,
        };
        assert_eq!(build_output_path(&item), PathBuf::from("slug/slugify.md"));
    }

    #[test]
    fn test_build_output_path_method() {
        let item = DocItem {
            name: "Config::from_file".to_string(),
            kind: DocKind::Method,
            docs: None,
            signature: "".to_string(),
            source: None,
            location: SourceLocation {
                file: PathBuf::from("config.rs"),
                start_line: None,
                end_line: None,
                repo_url: None,
            },
            module_path: vec!["config".to_string()],
            container: Some("Config".to_string()),
        };
        assert_eq!(
            build_output_path(&item),
            PathBuf::from("config/Config/from_file.md")
        );
    }

    #[test]
    fn test_build_output_path_nested_module() {
        let item = DocItem {
            name: "markdown::citations::render_references".to_string(),
            kind: DocKind::Function,
            docs: None,
            signature: "".to_string(),
            source: None,
            location: SourceLocation {
                file: PathBuf::from("markdown/citations.rs"),
                start_line: None,
                end_line: None,
                repo_url: None,
            },
            module_path: vec!["markdown".to_string(), "citations".to_string()],
            container: None,
        };
        assert_eq!(
            build_output_path(&item),
            PathBuf::from("markdown/citations/render_references.md")
        );
    }

    #[test]
    fn test_is_test_path() {
        assert!(is_test_path(Path::new("tests/foo.rs")));
        assert!(is_test_path(Path::new("src/__tests__/bar.ts")));
        assert!(is_test_path(Path::new("foo_test.rs")));
        assert!(is_test_path(Path::new("test_foo.py")));
        assert!(!is_test_path(Path::new("src/lib.rs")));
        assert!(!is_test_path(Path::new("src/config.rs")));
    }

    #[test]
    fn test_build_frontmatter() {
        let item = DocItem {
            name: "config::Config::from_file".to_string(),
            kind: DocKind::Method,
            docs: Some("Load configuration from a YAML file.".to_string()),
            signature: "pub fn from_file(path: &Path) -> Result<Self>".to_string(),
            source: None,
            location: SourceLocation {
                file: PathBuf::from("config.rs"),
                start_line: Some(42),
                end_line: Some(50),
                repo_url: None,
            },
            module_path: vec!["config".to_string()],
            container: Some("Config".to_string()),
        };

        let fm = build_frontmatter(&item, "rust");
        assert_eq!(fm.title, "config::Config::from_file");
        assert_eq!(fm.note_type, Some("doc".to_string()));
        assert!(fm.tags.contains(&"rust".to_string()));
        assert!(fm.tags.contains(&"api".to_string()));
        assert!(fm.tags.contains(&"kind:method".to_string()));
        assert!(fm.aliases.contains(&"config::Config::from_file".to_string()));
        assert_eq!(fm.summary, Some("Load configuration from a YAML file.".to_string()));
    }

    #[test]
    fn test_build_body() {
        let item = DocItem {
            name: "slugify".to_string(),
            kind: DocKind::Function,
            docs: Some("Convert a string to a URL-safe slug.".to_string()),
            signature: "pub fn slugify(s: &str) -> String".to_string(),
            source: Some("pub fn slugify(s: &str) -> String {\n    // ...\n}".to_string()),
            location: SourceLocation {
                file: PathBuf::from("slug.rs"),
                start_line: Some(10),
                end_line: Some(15),
                repo_url: Some("https://github.com/user/repo".to_string()),
            },
            module_path: vec!["slug".to_string()],
            container: None,
        };

        let body = build_body(&item, "rust");
        // Body now contains just the docs and collapsible source
        assert!(body.contains("Convert a string to a URL-safe slug."));
        assert!(body.contains("<details>"));
        assert!(body.contains("<summary>Source</summary>"));
        assert!(body.contains("```rust\npub fn slugify"));
    }

    #[test]
    fn test_available_adapters() {
        let adapters = available_adapters();
        #[cfg(feature = "rust")]
        assert!(adapters.contains(&"rust"));
    }

    #[test]
    fn test_adapter_by_name() {
        #[cfg(feature = "rust")]
        {
            let adapter = adapter_by_name("rust");
            assert!(adapter.is_some());
            assert_eq!(adapter.unwrap().name(), "rust");
        }

        let unknown = adapter_by_name("unknown");
        assert!(unknown.is_none());
    }
}
