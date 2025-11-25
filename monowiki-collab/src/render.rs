//! Incremental single-file rendering for live preview.
//!
//! Caches site context after the initial full build and allows
//! re-rendering individual notes without rebuilding the entire site.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use askama::Template;
use chrono::Datelike;
use tokio::sync::RwLock;
use tracing::{debug, info};

use monowiki_core::{
    bibliography::BibliographyStore,
    frontmatter::parse_frontmatter,
    markdown::{citations::CitationContext, MarkdownProcessor},
    Config, SiteIndex,
};
use monowiki_render::templates::{BacklinkEntry, PostTemplate};

/// Cached rendering context for incremental builds.
pub struct RenderCache {
    config: Config,
    slug_map: HashMap<String, String>,
    processor: MarkdownProcessor,
    bibliography_store: BibliographyStore,
    site_index: SiteIndex,
}

impl RenderCache {
    /// Create a new render cache from site config.
    /// Call `load_site_index()` after initial build to populate.
    pub fn new(config: Config) -> Self {
        let mut bibliography_store = BibliographyStore::new();
        bibliography_store.preload_paths(&config.bibliography_paths());

        Self {
            config,
            slug_map: HashMap::new(),
            processor: MarkdownProcessor::new(),
            bibliography_store,
            site_index: SiteIndex::default(),
        }
    }

    /// Load site index and build slug map after a full build.
    pub fn load_site_index(&mut self, index: SiteIndex) {
        let base_url = self.config.normalized_base_url();

        // Build slug map from all notes
        self.slug_map.clear();
        for note in &index.notes {
            let href = format!("{}{}", base_url, note.output_rel_path());
            self.slug_map.insert(note.slug.clone(), href.clone());
            // Aliases also resolve to the same target
            for alias in &note.aliases {
                self.slug_map
                    .insert(monowiki_core::slugify(alias), href.clone());
            }
        }

        self.site_index = index;
        info!(
            notes = self.site_index.notes.len(),
            slugs = self.slug_map.len(),
            "render cache loaded"
        );
    }

    /// Render a single note from markdown to HTML and write to output.
    pub fn render_single(&mut self, slug: &str, markdown: &str) -> Result<String> {
        let (frontmatter, body) = parse_frontmatter(markdown)
            .context("failed to parse frontmatter")?;

        // Get bibliography if configured
        let bibliography_paths = self.bibliography_paths(&frontmatter);
        let bibliography = self.bibliography_store.collect(&bibliography_paths);
        let citation_ctx = if bibliography.is_empty() {
            None
        } else {
            Some(CitationContext {
                bibliography: &bibliography,
            })
        };

        // Convert markdown to HTML
        let base_url = self.config.normalized_base_url();
        let (content_html, _outgoing_links, toc_html) = self.processor.convert(
            &body,
            &self.slug_map,
            &base_url,
            frontmatter.typst_preamble.as_deref(),
            citation_ctx.as_ref(),
        );

        // Get backlinks from cached graph
        let backlinks: Vec<BacklinkEntry> = self
            .site_index
            .graph
            .backlinks(slug)
            .iter()
            .filter_map(|source_slug| {
                self.site_index
                    .notes
                    .iter()
                    .find(|n| n.slug == *source_slug)
                    .map(|note| BacklinkEntry {
                        url: format!("{}{}", base_url, note.output_rel_path()),
                        title: note.title.clone(),
                    })
            })
            .collect();

        // Build title (from frontmatter or slug)
        let title = if frontmatter.title.trim().is_empty() {
            slug.to_string()
        } else {
            frontmatter.title.clone()
        };

        // Build template (matching CLI build.rs pattern)
        let template = PostTemplate {
            title: title.clone(),
            description: frontmatter
                .description
                .clone()
                .unwrap_or_else(|| title.clone()),
            date: frontmatter.date.clone(),
            updated: frontmatter.updated.clone(),
            tags: frontmatter.tags.clone(),
            content: content_html,
            toc_html,
            site_title: self.config.site.title.clone(),
            site_author: self.config.site.author.clone(),
            year: chrono::Utc::now().year(),
            nav_home: format!("{}index.html", base_url),
            nav_about: format!("{}about.html", base_url),
            nav_github: self.config.site.url.clone(),
            has_about: false, // TODO: Check if about.html exists
            has_github: true,
            css_path: base_url.to_string(), // Asset prefix
            backlinks,
            base_url: base_url.to_string(),
            slug: slug.to_string(),
            source: Some(body),
        };

        // Render to HTML
        let html = template.render().context("failed to render template")?;

        // Write to output directory (flatten directory structure like full build)
        let filename = slug.rsplit('/').next().unwrap_or(slug);
        let output_path = self.config.output_dir().join(format!("{}.html", filename));
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&output_path, &html)?;

        debug!(slug, path = %output_path.display(), "rendered single note");
        Ok(html)
    }

    fn bibliography_paths(
        &self,
        frontmatter: &monowiki_core::Frontmatter,
    ) -> Vec<PathBuf> {
        let mut paths = self.config.bibliography_paths();
        for extra in &frontmatter.bibliography {
            if extra.trim().is_empty() {
                continue;
            }
            let path = self
                .config
                .resolve_relative(std::path::Path::new(extra.trim()));
            paths.push(path);
        }
        paths
    }

    /// Get the config reference.
    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Thread-safe wrapper around RenderCache.
#[derive(Clone)]
pub struct SharedRenderCache(Arc<RwLock<Option<RenderCache>>>);

impl SharedRenderCache {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(None)))
    }

    /// Initialize the cache with a config.
    pub async fn initialize(&self, config: Config) {
        let mut guard = self.0.write().await;
        *guard = Some(RenderCache::new(config));
    }

    /// Load site index after a full build.
    pub async fn load_site_index(&self, index: SiteIndex) {
        let mut guard = self.0.write().await;
        if let Some(cache) = guard.as_mut() {
            cache.load_site_index(index);
        }
    }

    /// Render a single note, returning the HTML.
    pub async fn render_single(&self, slug: &str, markdown: &str) -> Result<String> {
        let mut guard = self.0.write().await;
        match guard.as_mut() {
            Some(cache) => cache.render_single(slug, markdown),
            None => anyhow::bail!("render cache not initialized"),
        }
    }
}

impl Default for SharedRenderCache {
    fn default() -> Self {
        Self::new()
    }
}
