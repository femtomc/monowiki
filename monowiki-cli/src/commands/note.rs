//! Fetch a single note in structured form.

use crate::NoteFormat;
use anyhow::{Context, Result};
use monowiki_core::{slugify, Config, SiteBuilder};
use serde_json::json;
use std::path::Path;

/// Fetch a single note and render it in the requested format.
pub fn show_note(config_path: &Path, slug: &str, format: NoteFormat, with_links: bool) -> Result<()> {
    let config = Config::from_file(config_path).context("Failed to load configuration")?;
    let base_url = config.normalized_base_url();

    let builder = SiteBuilder::new(config.clone());
    let site_index = builder.build().context("Failed to build site index")?;

    let note = find_note(&site_index, slug)
        .with_context(|| format!("Note '{}' not found (slug, alias, or permalink)", slug))?;

    let backlinks = if with_links {
        site_index.graph.backlinks(&note.slug)
    } else {
        Vec::new()
    };

    match format {
        NoteFormat::Json => {
            let payload = json!({
                "slug": note.slug,
                "title": note.title,
                "url": note.url_with_base(&base_url),
                "type": note.note_type.as_str(),
                "tags": note.tags,
                "date": note.date.map(|d| d.format("%Y-%m-%d").to_string()),
                "updated": note.updated.map(|d| d.format("%Y-%m-%d").to_string()),
                "frontmatter": note.frontmatter,
                "content_html": note.content_html,
                "toc_html": note.toc_html,
                "raw_body": note.raw_body,
                "preview": note.preview,
                "outgoing": note.outgoing_links,
                "backlinks": backlinks,
            });
            println!("{}", serde_json::to_string_pretty(&payload)?);
        }
        NoteFormat::Markdown => {
            let fm = serde_yaml::to_string(&note.frontmatter).unwrap_or_default();
            let body = note.raw_body.clone().unwrap_or_default();
            println!("---\n{}---\n{}", fm, body);
        }
        NoteFormat::Html => {
            println!("{}", note.content_html);
        }
        NoteFormat::Frontmatter => {
            let fm = serde_yaml::to_string(&note.frontmatter).unwrap_or_default();
            println!("---\n{}---", fm);
        }
        NoteFormat::Raw => {
            if let Some(body) = &note.raw_body {
                println!("{body}");
            } else {
                println!("{}", note.content_html);
            }
        }
    }

    Ok(())
}

fn normalize_slugish(s: &str) -> String {
    let trimmed = s.trim().trim_matches('/');
    let without_html = trimmed.strip_suffix(".html").unwrap_or(trimmed);
    slugify(without_html)
}

fn find_note<'a>(
    site_index: &'a monowiki_core::SiteIndex,
    query: &str,
) -> Option<&'a monowiki_core::Note> {
    let normalized = normalize_slugish(query);

    site_index
        .find_by_slug(&normalized)
        .or_else(|| site_index.find_by_alias(&normalized))
        .or_else(|| site_index.find_by_permalink(query))
}
