//! Site building logic - orchestrates parsing, rendering, and output.

use crate::{
    bibliography::BibliographyStore,
    config::Config,
    frontmatter::parse_frontmatter,
    markdown::{citations::CitationContext, MarkdownProcessor},
    models::*,
    search::section_digests_from_html,
    slug::slugify,
};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum BuildError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Frontmatter error: {0}")]
    Frontmatter(#[from] crate::frontmatter::FrontmatterError),

    #[error("Duplicate slug: {0}")]
    DuplicateSlug(String),
}

/// Main site builder
pub struct SiteBuilder {
    config: Config,
    processor: MarkdownProcessor,
}

impl SiteBuilder {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            processor: MarkdownProcessor::new(),
        }
    }

    /// Build the entire site
    pub fn build(&self) -> Result<SiteIndex, BuildError> {
        // Create output directory
        fs::create_dir_all(self.config.output_dir())?;

        // Discover all markdown files
        let markdown_files = self.discover_markdown_files()?;

        tracing::info!("Found {} markdown files", markdown_files.len());

        let mut bibliography_store = BibliographyStore::new();
        bibliography_store.preload_paths(&self.config.bibliography_paths());

        // Parse all notes (first pass - without link resolution)
        let mut notes = Vec::new();
        let mut slug_map: HashMap<String, String> = HashMap::new();
        let base_url = self.config.normalized_base_url();
        let mut diagnostics: Vec<Diagnostic> = Vec::new();

        for file_path in &markdown_files {
            match self.parse_note(file_path) {
                Ok(note) => {
                    // Check for duplicate slugs
                    if slug_map.contains_key(&note.slug) {
                        tracing::warn!("Duplicate slug: {}", note.slug);
                        return Err(BuildError::DuplicateSlug(note.slug.clone()));
                    }

                    let href = format!("{}{}", base_url, note.output_rel_path());
                    slug_map.insert(note.slug.clone(), href.clone());
                    // Aliases also resolve to the same target
                    for alias in &note.aliases {
                        let alias_slug = slugify(alias);
                        if let Some(existing) = slug_map.get(&alias_slug) {
                            // Only flag if the alias would point somewhere else
                            if existing != &href {
                                diagnostics.push(Diagnostic {
                                    code: "alias.duplicate".to_string(),
                                    message: format!(
                                        "Alias '{}' on '{}' conflicts with an existing target",
                                        alias, note.slug
                                    ),
                                    severity: DiagnosticSeverity::Warning,
                                    note_slug: Some(note.slug.clone()),
                                    source_path: note.source_path.clone(),
                                    context: Some(alias_slug.clone()),
                                });
                            }
                        } else {
                            slug_map.insert(alias_slug, href.clone());
                        }
                    }
                    notes.push(note);
                }
                Err(e) => {
                    tracing::error!("Failed to parse {:?}: {}", file_path, e);
                    // Continue with other files
                }
            }
        }

        // Second pass - render markdown with link resolution
        for (idx, note) in notes.iter_mut().enumerate() {
            let markdown = fs::read_to_string(&markdown_files[idx])?;
            let (frontmatter, body) = parse_frontmatter(&markdown)?;

            let bibliography_paths = self.bibliography_paths(&frontmatter);
            let bibliography = bibliography_store.collect(&bibliography_paths);
            let citation_ctx = if bibliography.is_empty() {
                None
            } else {
                Some(CitationContext {
                    bibliography: &bibliography,
                })
            };

            let (html, outgoing_links, toc_html, mut note_diags) = self.processor.convert(
                &body,
                &slug_map,
                &base_url,
                frontmatter.typst_preamble.as_deref(),
                citation_ctx.as_ref(),
                Some(&note.slug),
                note.source_path.as_deref(),
            );
            note.content_html = html;
            note.outgoing_links = outgoing_links;
            note.toc_html = toc_html;
            note.raw_body = Some(body);
            diagnostics.append(&mut note_diags);
        }

        // Build link graph
        let mut graph = LinkGraph::new();
        for note in &notes {
            if note.note_type == NoteType::Comment {
                continue;
            }
            for target in &note.outgoing_links {
                graph.add_link(&note.slug, target);
            }
        }

        // Carry over bibliography load diagnostics
        diagnostics.extend(bibliography_store.take_diagnostics());

        tracing::info!("Built site index with {} notes", notes.len());

        // Collect comments and resolve anchors
        let comments = collect_comments(&notes);

        Ok(SiteIndex {
            notes,
            graph,
            diagnostics,
            comments,
        })
    }

    /// Discover all markdown files in the vault
    fn discover_markdown_files(&self) -> Result<Vec<PathBuf>, BuildError> {
        let vault_dir = self.config.vault_dir();
        let mut files = Vec::new();
        let ignore_patterns = compile_ignore_patterns(&self.config.ignore_patterns);

        for entry in WalkDir::new(&vault_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            if let Some(ext) = entry.path().extension() {
                if ext == "md" {
                    // Skip ignored paths
                    let rel = entry
                        .path()
                        .strip_prefix(&vault_dir)
                        .unwrap_or(entry.path())
                        .to_string_lossy()
                        .to_string();
                    if should_ignore(&rel, &ignore_patterns) {
                        tracing::debug!("Ignoring {} due to ignore_patterns", rel);
                        continue;
                    }

                    files.push(entry.path().to_path_buf());
                }
            }
        }

        Ok(files)
    }

    /// Parse a single markdown file into a Note (without rendering markdown yet)
    fn parse_note(&self, path: &Path) -> Result<Note, BuildError> {
        let content = fs::read_to_string(path)?;
        let (frontmatter, _body) = parse_frontmatter(&content)?;

        // Fall back to filename when title/frontmatter is missing (e.g., pure markdown)
        let mut title = frontmatter.title.clone();
        if title.trim().is_empty() {
            title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string();
        }

        // Determine slug (from frontmatter or filename)
        let slug = frontmatter.slug.clone().unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| slugify(s))
                .unwrap_or_else(|| slugify(&frontmatter.title))
        });

        // Determine note type
        let note_type = frontmatter
            .note_type
            .as_ref()
            .and_then(|t| NoteType::from_str(t))
            .unwrap_or(NoteType::Essay);

        // Parse dates
        let date = frontmatter
            .date
            .as_ref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let updated = frontmatter
            .updated
            .as_ref()
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        // Capture source path relative to vault root
        let vault_dir = self.config.vault_dir();
        let source_path = path
            .strip_prefix(&vault_dir)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string());

        Ok(Note {
            slug,
            title,
            content_html: String::new(), // Will be filled in second pass
            frontmatter: frontmatter.clone(),
            note_type,
            tags: frontmatter.tags.clone(),
            date,
            updated,
            aliases: frontmatter.aliases.clone(),
            permalink: frontmatter.permalink.clone(),
            outgoing_links: Vec::new(), // Will be filled in second pass
            preview: frontmatter.summary.clone(),
            toc_html: None, // TODO: Generate TOC
            raw_body: None,
            source_path,
        })
    }
}

impl SiteBuilder {
    fn bibliography_paths(&self, frontmatter: &Frontmatter) -> Vec<PathBuf> {
        let mut paths = self.config.bibliography_paths();
        for extra in &frontmatter.bibliography {
            if extra.trim().is_empty() {
                continue;
            }
            let path = self.config.resolve_relative(Path::new(extra.trim()));
            paths.push(path);
        }
        paths
    }
}

fn compile_ignore_patterns(patterns: &[String]) -> Vec<Regex> {
    let mut compiled = Vec::new();
    for pat in patterns {
        match Regex::new(pat) {
            Ok(re) => compiled.push(re),
            Err(err) => tracing::warn!("Invalid ignore pattern '{}': {}", pat, err),
        }
    }
    compiled
}

fn should_ignore(path: &str, ignores: &[Regex]) -> bool {
    ignores.iter().any(|re| re.is_match(path))
}

/// Extract comments/annotations from notes of type Comment and resolve anchors.
fn collect_comments(notes: &[Note]) -> Vec<Comment> {
    // Build lookup of target notes by slug for resolution
    let mut note_map: HashMap<String, &Note> = HashMap::new();
    for note in notes {
        if note.note_type != NoteType::Comment {
            note_map.insert(note.slug.clone(), note);
        }
    }

    let mut comments = Vec::new();
    for note in notes.iter().filter(|n| n.note_type == NoteType::Comment) {
        let target_slug = note.frontmatter.target_slug.clone();
        let target_anchor = note.frontmatter.target_anchor.clone();
        let quote = note.frontmatter.quote.clone();

        let (resolved_anchor, resolved) = resolve_anchor(
            target_slug.as_deref(),
            target_anchor.as_deref(),
            quote.as_deref(),
            &note_map,
        );

        let status = note
            .frontmatter
            .status
            .as_deref()
            .and_then(|s| match s.to_lowercase().as_str() {
                "resolved" => Some(CommentStatus::Resolved),
                "open" => Some(CommentStatus::Open),
                _ => None,
            })
            .unwrap_or_default();

        comments.push(Comment {
            id: note.slug.clone(),
            target_slug: target_slug.clone(),
            target_anchor: target_anchor.clone(),
            resolved_anchor,
            resolved,
            git_ref: note.frontmatter.git_ref.clone(),
            quote,
            author: note.frontmatter.author.clone(),
            tags: note.tags.clone(),
            status,
            content_html: note.content_html.clone(),
            source_path: note.source_path.clone(),
            note_slug: note.slug.clone(),
        });
    }

    comments
}

fn resolve_anchor(
    target_slug: Option<&str>,
    target_anchor: Option<&str>,
    quote: Option<&str>,
    note_map: &HashMap<String, &Note>,
) -> (Option<String>, bool) {
    let Some(slug) = target_slug else {
        return (None, false);
    };
    let Some(target) = note_map.get(slug) else {
        return (None, false);
    };

    let sections = section_digests_from_html(&target.slug, &target.title, &target.content_html);

    // 1) Exact match on stable section id
    if let Some(anchor) = target_anchor {
        if sections
            .iter()
            .any(|s| s.section_id == anchor || s.anchor_id.as_deref() == Some(anchor))
        {
            return (Some(anchor.to_string()), true);
        }
    }

    // 2) Fuzzy quote match: find section containing the quote in its content
    if let Some(q) = quote {
        if let Some(section_id) = find_section_by_quote(target, q) {
            return (Some(section_id), true);
        }
    }

    (None, false)
}

fn find_section_by_quote(note: &Note, quote: &str) -> Option<String> {
    let entries = crate::search::build_search_index(
        &note.slug,
        &note.title,
        &note.content_html,
        &note.tags,
        note.note_type.as_str(),
        "/",
    );
    let quote_norm = quote.to_lowercase();
    for entry in entries {
        if entry.content.to_lowercase().contains(&quote_norm) {
            return Some(entry.section_id);
        }
    }
    None
}
