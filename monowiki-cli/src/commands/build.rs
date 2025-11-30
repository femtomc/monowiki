//! Build command implementation.

use crate::cache;
use anyhow::{Context, Result};
use askama::Template;
use chrono::{Datelike, NaiveDate};
use include_dir::{include_dir, Dir};
use monowiki_core::{Config, SiteBuilder};
use monowiki_render::{BacklinkEntry, DirectoryNode, FileNode, NotFoundTemplate, PostTemplate};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

// Embed the theme bundle at compile time so it's available after cargo install
static THEME_BUNDLE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../theme/dist");

// Embed static assets (CSS, fonts) at compile time
static STATIC_ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../static");

/// Build the static site (writes output) and discard the in-memory index
pub fn build_site(config_path: &Path) -> Result<()> {
    build_site_with_index(config_path).map(|_| ())
}

/// Build the static site and return the in-memory index alongside the loaded config
pub fn build_site_with_index(config_path: &Path) -> Result<(Config, monowiki_core::SiteIndex)> {
    tracing::info!("Loading config from {:?}", config_path);
    let config = Config::from_file(config_path).context("Failed to load configuration")?;
    build_site_with_config(config)
}

/// Build the site from an already loaded config, writing output and returning the index.
pub fn build_site_with_config(config: Config) -> Result<(Config, monowiki_core::SiteIndex)> {
    let base_url = config.normalized_base_url();

    tracing::info!("Building site: {}", config.site.title);

    // Build the site
    let builder = SiteBuilder::new(config.clone());
    let site_index = builder.build().context("Failed to build site")?;

    tracing::info!("Parsed {} notes", site_index.notes.len());

    // Create output directory
    let output_dir = config.output_dir();
    fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

    // Render individual note pages
    for note in &site_index.notes {
        // Skip drafts
        if note.is_draft() || note.note_type == monowiki_core::NoteType::Comment {
            tracing::debug!("Skipping draft: {}", note.title);
            continue;
        }

        render_note_page(&config, note, &site_index, &base_url)?;
    }

    // Render 404 page
    render_404_page(&config, &base_url)?;

    // Generate JSON artifacts
    generate_previews_json(&config, &site_index, &base_url)?;
    generate_index_json(&config, &site_index, &base_url)?;
    if config.enable_backlinks {
        generate_graph_json(&config, &site_index, &base_url)?;
    } else {
        tracing::info!("Backlinks disabled; skipping graph.json");
    }

    // Syndication artifacts
    if config.enable_rss {
        generate_rss(&config, &site_index, &base_url)?;
    } else {
        tracing::info!("RSS disabled; skipping rss.xml");
    }

    if config.enable_sitemap {
        generate_sitemap(&config, &site_index, &base_url)?;
    } else {
        tracing::info!("Sitemap disabled; skipping sitemap.xml");
    }

    // Copy CSS/JS assets
    copy_assets(&config)?;

    let non_draft_count = site_index.notes.iter().filter(|n| !n.is_draft()).count();

    tracing::info!("✓ Built {} pages", non_draft_count);
    tracing::info!("✓ Output written to {:?}", output_dir);

    if let Err(err) = cache::write_site_index_cache(&config, &site_index) {
        tracing::warn!("Failed to write site index cache: {}", err);
    }

    Ok((config, site_index))
}

/// Render a single note page
fn render_note_page(
    config: &Config,
    note: &monowiki_core::Note,
    site_index: &monowiki_core::SiteIndex,
    base_url: &str,
) -> Result<()> {
    // Get backlinks
    let backlinks: Vec<BacklinkEntry> = if config.enable_backlinks {
        let backlink_slugs = site_index.graph.backlinks(&note.slug);
        backlink_slugs
            .iter()
            .filter_map(|slug| site_index.find_by_slug(slug))
            .map(|n| BacklinkEntry {
                url: n.url_with_base(base_url),
                title: n.title.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };

    // Format dates
    let date = note.date.as_ref().map(|d| d.format("%Y-%m-%d").to_string());
    let updated = note
        .updated
        .as_ref()
        .map(|d| d.format("%Y-%m-%d").to_string());

    // Expand {{directory_tree}} macro if present
    let content = expand_macros(&note.content_html, site_index, base_url);

    let template = PostTemplate {
        title: note.title.clone(),
        description: note
            .frontmatter
            .description
            .clone()
            .unwrap_or_else(|| note.title.clone()),
        date,
        updated,
        tags: note.tags.clone(),
        content,
        toc_html: note.toc_html.clone(),
        site_title: config.site.title.clone(),
        site_author: config.site.author.clone(),
        year: chrono::Utc::now().year(),
        nav_home: format!("{}index.html", base_url),
        nav_about: format!("{}about.html", base_url),
        nav_github: config.site.url.clone(),
        has_about: false, // TODO: Check if about.html exists
        has_github: true,
        css_path: base_url.to_string(), // Asset prefix
        backlinks,
        base_url: base_url.to_string(),
        slug: note.slug.clone(),
        source: note.raw_body.clone(),
    };

    let html = template
        .render()
        .context("Failed to render post template")?;

    let output_path = config.output_dir().join(note.output_rel_path());
    fs::write(&output_path, html).with_context(|| format!("Failed to write {:?}", output_path))?;

    tracing::debug!("Rendered: {}", note.slug);

    Ok(())
}

/// Build a directory tree structure from notes with arbitrary nesting
fn build_directory_tree(notes: &[&monowiki_core::Note], base_url: &str) -> Vec<DirectoryNode> {
    // Build a hierarchical tree structure
    let mut root_dirs: HashMap<String, DirectoryNode> = HashMap::new();

    for note in notes {
        if let Some(source_path) = &note.source_path {
            let path_parts: Vec<&str> = source_path.split('/').collect();

            if path_parts.is_empty() {
                continue;
            }

            let file_name = path_parts[path_parts.len() - 1];
            let file_node = FileNode {
                name: file_name.to_string(),
                url: note.url_with_base(base_url),
                title: note.title.clone(),
                note_type: note.note_type.as_str().to_uppercase(),
            };

            if path_parts.len() == 1 {
                // File at root level
                let root_dir = root_dirs
                    .entry(String::new())
                    .or_insert_with(|| DirectoryNode {
                        name: String::new(),
                        files: Vec::new(),
                        subdirs: Vec::new(),
                    });
                root_dir.files.push(file_node);
            } else {
                // File in nested directories
                insert_into_tree(
                    &mut root_dirs,
                    &path_parts[..path_parts.len() - 1],
                    file_node,
                );
            }
        }
    }

    // Convert to sorted vector
    sort_directory_tree(root_dirs)
}

/// Insert a file into a nested directory tree, creating directories as needed
fn insert_into_tree(
    tree: &mut HashMap<String, DirectoryNode>,
    dir_path: &[&str],
    file_node: FileNode,
) {
    if dir_path.is_empty() {
        return;
    }

    let first_dir = dir_path[0];
    let rest = &dir_path[1..];

    let dir_node = tree
        .entry(first_dir.to_string())
        .or_insert_with(|| DirectoryNode {
            name: first_dir.to_string(),
            files: Vec::new(),
            subdirs: Vec::new(),
        });

    if rest.is_empty() {
        // We're at the final directory level - add the file here
        dir_node.files.push(file_node);
    } else {
        // Continue recursing into subdirectories
        let mut subdir_map: HashMap<String, DirectoryNode> = HashMap::new();

        // Extract existing subdirs into a HashMap
        for subdir in dir_node.subdirs.drain(..) {
            subdir_map.insert(subdir.name.clone(), subdir);
        }

        // Recursively insert
        insert_into_tree(&mut subdir_map, rest, file_node);

        // Convert back to sorted vector
        dir_node.subdirs = sort_directory_tree(subdir_map);
    }
}

/// Convert a HashMap of directories into a sorted vector with sorted contents
fn sort_directory_tree(tree: HashMap<String, DirectoryNode>) -> Vec<DirectoryNode> {
    let mut directories: Vec<DirectoryNode> = tree.into_values().collect();

    // Sort directories by name
    directories.sort_by(|a, b| a.name.cmp(&b.name));

    // Sort files and subdirs within each directory
    for dir in &mut directories {
        dir.files.sort_by(|a, b| a.name.cmp(&b.name));
        // Subdirs are already sorted by the recursive calls
    }

    directories
}

/// Expand macros like {{directory_tree}} in content
fn expand_macros(content: &str, site_index: &monowiki_core::SiteIndex, base_url: &str) -> String {
    // Check for {{directory_tree}} macro (may be wrapped in <p> tags by markdown parser)
    if !content.contains("{{directory_tree}}") {
        return content.to_string();
    }

    // Build directory tree from published notes
    let published_notes: Vec<&monowiki_core::Note> =
        site_index.notes.iter().filter(|n| !n.is_draft()).collect();
    let directory_tree = build_directory_tree(&published_notes, base_url);

    // Render directory tree to HTML
    let tree_html = if directory_tree.is_empty() {
        "<p><em>No notes yet.</em></p>".to_string()
    } else {
        let mut html = String::from("<div class=\"directory-tree\">\n");
        for dir in &directory_tree {
            html.push_str(&dir.render_to_html());
        }
        html.push_str("</div>");
        html
    };

    // Replace the macro (handle both raw and paragraph-wrapped versions)
    content
        .replace("<p>{{directory_tree}}</p>", &tree_html)
        .replace("{{directory_tree}}", &tree_html)
}

/// Render the 404 error page
fn render_404_page(config: &Config, base_url: &str) -> Result<()> {
    let template = NotFoundTemplate {
        site_title: config.site.title.clone(),
        site_author: config.site.author.clone(),
        year: chrono::Utc::now().year(),
        nav_home: format!("{}index.html", base_url),
        nav_about: format!("{}about.html", base_url),
        nav_github: config.site.url.clone(),
        has_about: false,
        has_github: true,
        css_path: base_url.to_string(),
        base_url: base_url.to_string(),
    };

    let html = template.render().context("Failed to render 404 template")?;

    let output_path = config.output_dir().join("404.html");
    fs::write(&output_path, html).context("Failed to write 404.html")?;

    tracing::info!("Rendered 404 page");

    Ok(())
}

/// Generate previews.json for link previews
fn generate_previews_json(
    config: &Config,
    site_index: &monowiki_core::SiteIndex,
    base_url: &str,
) -> Result<()> {
    use serde_json::json;

    let mut previews = serde_json::Map::new();

    for note in &site_index.notes {
        if note.is_draft() || note.note_type == monowiki_core::NoteType::Comment {
            continue;
        }

        let url = note.output_rel_path();

        // Use TOC if available, otherwise fallback to description or first line
        let preview_text = if let Some(ref toc_html) = note.toc_html {
            toc_html.clone()
        } else {
            note.preview.clone().unwrap_or_else(|| {
                note.frontmatter
                    .description
                    .clone()
                    .unwrap_or_else(|| "No preview available".to_string())
            })
        };

        previews.insert(
            url,
            json!({
                "title": note.title,
                "preview": preview_text,
                "type": note.note_type.as_str(),
                "has_toc": note.toc_html.is_some(),
                "url": note.url_with_base(base_url),
            }),
        );
    }

    let output_path = config.output_dir().join("previews.json");
    let json = serde_json::to_string_pretty(&previews).context("Failed to serialize previews")?;
    fs::write(&output_path, json).context("Failed to write previews.json")?;

    tracing::info!("Generated previews.json");

    Ok(())
}

/// Generate index.json for search with section-level granularity
fn generate_index_json(
    config: &Config,
    site_index: &monowiki_core::SiteIndex,
    base_url: &str,
) -> Result<()> {
    let mut index = Vec::new();

    for note in &site_index.notes {
        if note.is_draft() || note.note_type == monowiki_core::NoteType::Comment {
            continue;
        }

        // Build section-level search entries
        let entries = monowiki_core::build_search_index(
            &note.slug,
            &note.title,
            &note.content_html,
            &note.tags,
            note.note_type.as_str(),
            base_url,
        );

        index.extend(entries);
    }

    let output_path = config.output_dir().join("index.json");
    let json = serde_json::to_string_pretty(&index).context("Failed to serialize search index")?;
    fs::write(&output_path, json).context("Failed to write index.json")?;

    tracing::info!("Generated index.json with {} search entries", index.len());

    Ok(())
}

/// Generate graph.json for backlinks visualization
fn generate_graph_json(
    config: &Config,
    site_index: &monowiki_core::SiteIndex,
    base_url: &str,
) -> Result<()> {
    use serde_json::json;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for note in &site_index.notes {
        if note.is_draft() {
            continue;
        }

        nodes.push(json!({
            "id": note.slug,
            "title": note.title,
            "type": note.note_type.as_str(),
            "url": note.output_rel_path(),
            "href": note.url_with_base(base_url),
        }));

        for target in &note.outgoing_links {
            edges.push(json!({
                "source": note.slug,
                "target": target,
            }));
        }
    }

    let graph = json!({
        "nodes": nodes,
        "edges": edges,
    });

    let output_path = config.output_dir().join("graph.json");
    let json = serde_json::to_string_pretty(&graph).context("Failed to serialize graph")?;
    fs::write(&output_path, json).context("Failed to write graph.json")?;

    tracing::info!("Generated graph.json");

    Ok(())
}

/// Copy CSS/JS assets to output
fn copy_assets(config: &Config) -> Result<()> {
    let output_dir = config.output_dir();
    // Copy built-in static assets (css, fonts, images, etc.)
    let static_dir = Path::new("static");
    if static_dir.exists() {
        copy_dir(static_dir, &output_dir)?;
        tracing::info!("Copied assets from local static/");
    } else {
        // Use embedded static assets (available after cargo install)
        extract_embedded_static(&output_dir)?;
        tracing::info!("Copied assets from embedded static bundle");
    }

    // Copy bundled theme JS
    let js_dest = output_dir.join("js");
    if js_dest.exists() {
        fs::remove_dir_all(&js_dest)
            .with_context(|| format!("Failed to clean existing {:?}", js_dest))?;
    }
    fs::create_dir_all(&js_dest)?;

    // Try to use local theme/dist first (for development), fall back to embedded
    let theme_dist = Path::new("theme/dist");
    if theme_dist.exists() {
        copy_theme_dist(theme_dist, &js_dest)?;
        tracing::info!("Copied theme bundle from local theme/dist");
    } else {
        // Use embedded theme assets (available after cargo install)
        extract_embedded_theme(&js_dest)?;
        tracing::info!("Copied theme bundle from embedded assets");
    }

    // Copy custom theme directory if provided
    if let Some(theme_dir) = config.theme_dir() {
        if theme_dir.exists() {
            copy_dir(&theme_dir, &output_dir)?;
            tracing::info!("Copied custom theme from {:?}", theme_dir);
        } else {
            tracing::warn!("Configured theme path {:?} does not exist", theme_dir);
        }
    }

    // Apply theme overrides (post-copy) if provided
    if let Some(overrides_dir) = config.theme_overrides_dir() {
        if overrides_dir.exists() {
            copy_dir(&overrides_dir, &output_dir)?;
            tracing::info!("Applied theme overrides from {:?}", overrides_dir);
        } else {
            tracing::warn!(
                "Configured theme_overrides path {:?} does not exist",
                overrides_dir
            );
        }
    }

    Ok(())
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    for entry in WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let relative = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let target = dest.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), &target)
            .with_context(|| format!("Failed to copy {:?} to {:?}", entry.path(), target))?;
    }
    Ok(())
}

fn copy_theme_dist(src: &Path, dest: &Path) -> Result<()> {
    for entry in WalkDir::new(src)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if entry.path().extension().and_then(|ext| ext.to_str()) == Some("map") {
            continue;
        }

        let relative = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let target = dest.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), &target)
            .with_context(|| format!("Failed to copy {:?} to {:?}", entry.path(), target))?;
    }

    Ok(())
}

fn extract_embedded_theme(dest: &Path) -> Result<()> {
    // Extract all files from embedded theme, skipping .map files
    for file in THEME_BUNDLE.files() {
        let path = file.path();

        // Skip sourcemap files
        if path.extension().and_then(|e| e.to_str()) == Some("map") {
            continue;
        }

        let target = dest.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&target, file.contents())
            .with_context(|| format!("Failed to write embedded theme file to {:?}", target))?;
    }

    Ok(())
}

fn extract_embedded_static(dest: &Path) -> Result<()> {
    // Extract all files from embedded static directory (CSS, fonts, etc.)
    // The include_dir crate stores full paths relative to the embedded root
    for entry in STATIC_ASSETS.entries() {
        extract_entry(entry, dest)?;
    }
    Ok(())
}

fn extract_entry(entry: &include_dir::DirEntry, dest: &Path) -> Result<()> {
    match entry {
        include_dir::DirEntry::Dir(dir) => {
            for sub_entry in dir.entries() {
                extract_entry(sub_entry, dest)?;
            }
        }
        include_dir::DirEntry::File(file) => {
            let target = dest.join(file.path());
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target, file.contents())
                .with_context(|| format!("Failed to write embedded static file to {:?}", target))?;
        }
    }
    Ok(())
}

/// Generate RSS feed (rss.xml)
fn generate_rss(
    config: &Config,
    site_index: &monowiki_core::SiteIndex,
    base_url: &str,
) -> Result<()> {
    let mut items = String::new();
    let mut notes: Vec<_> = site_index.notes.iter().filter(|n| !n.is_draft()).collect();

    notes.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.updated.cmp(&a.updated)));

    for note in notes {
        let link = absolute_url(&config.site.url, base_url, &note.output_rel_path());
        let title = escape_xml(&note.title);
        let description = escape_xml(
            note.frontmatter
                .description
                .as_ref()
                .or_else(|| note.frontmatter.summary.as_ref())
                .unwrap_or(&note.title),
        );

        let pub_date = note
            .updated
            .or(note.date)
            .and_then(|d| naive_to_rfc2822(&d));

        items.push_str(&format!(
            "<item><title>{}</title><link>{}</link><guid>{}</guid><description>{}</description>",
            title, link, link, description
        ));
        if let Some(pd) = pub_date {
            items.push_str(&format!("<pubDate>{}</pubDate>", pd));
        }
        items.push_str("</item>");
    }

    let channel_link = absolute_url(&config.site.url, base_url, "");
    let rss = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>{}</title>
    <link>{}</link>
    <description>{}</description>
    {}
  </channel>
</rss>
"#,
        escape_xml(&config.site.title),
        channel_link,
        escape_xml(&config.site.description),
        items
    );

    fs::write(config.output_dir().join("rss.xml"), rss)?;
    tracing::info!("Generated rss.xml");
    Ok(())
}

/// Generate sitemap.xml
fn generate_sitemap(
    config: &Config,
    site_index: &monowiki_core::SiteIndex,
    base_url: &str,
) -> Result<()> {
    let mut urls = String::new();

    // Index
    urls.push_str(&format!(
        "<url><loc>{}</loc></url>",
        absolute_url(&config.site.url, base_url, "index.html")
    ));

    for note in &site_index.notes {
        if note.is_draft() {
            continue;
        }
        let loc = absolute_url(&config.site.url, base_url, &note.output_rel_path());
        let lastmod = note.updated.or(note.date);
        urls.push_str("<url>");
        urls.push_str(&format!("<loc>{}</loc>", loc));
        if let Some(date) = lastmod {
            urls.push_str(&format!("<lastmod>{}</lastmod>", date.format("%Y-%m-%d")));
        }
        urls.push_str("</url>");
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{}
</urlset>
"#,
        urls
    );

    fs::write(config.output_dir().join("sitemap.xml"), xml)?;
    tracing::info!("Generated sitemap.xml");
    Ok(())
}

fn absolute_url(site_url: &str, base_url: &str, rel: &str) -> String {
    let root = site_url.trim_end_matches('/').to_string();
    let mut base = base_url.trim_matches('/').to_string();
    if !base.is_empty() {
        base = format!("/{}", base);
    }
    let rel_clean = rel.trim_start_matches('/');
    let joined = if rel_clean.is_empty() {
        format!("{}{}", root, base)
    } else {
        format!("{}{}/{}", root, base, rel_clean)
    };
    joined.replace("//", "/").replace(":/", "://")
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn naive_to_rfc2822(date: &NaiveDate) -> Option<String> {
    let datetime = date.and_hms_opt(0, 0, 0)?;
    Some(datetime.and_utc().to_rfc2822())
}
