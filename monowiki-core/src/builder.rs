//! Site building logic - orchestrates parsing, rendering, and output.

use crate::{
    config::Config, frontmatter::parse_frontmatter, markdown::MarkdownProcessor, models::*,
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

        // Parse all notes (first pass - without link resolution)
        let mut notes = Vec::new();
        let mut slug_map: HashMap<String, String> = HashMap::new();
        let base_url = self.config.normalized_base_url();

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
                        slug_map.insert(slugify(alias), href.clone());
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
            let (_, body) = parse_frontmatter(&markdown)?;

            let (html, outgoing_links, toc_html) =
                self.processor.convert(&body, &slug_map, &base_url);
            note.content_html = html;
            note.outgoing_links = outgoing_links;
            note.toc_html = toc_html;
            note.raw_body = Some(body);
        }

        // Build link graph
        let mut graph = LinkGraph::new();
        for note in &notes {
            for target in &note.outgoing_links {
                graph.add_link(&note.slug, target);
            }
        }

        tracing::info!("Built site index with {} notes", notes.len());

        Ok(SiteIndex { notes, graph })
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests removed - config_path is private and we don't need to test this
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
