//! Comment/annotation helpers.

use crate::cache::load_or_build_site_index;
use anyhow::{Context, Result};
use chrono::Utc;
use monowiki_core::{CommentStatus, Config};
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Serialize)]
pub struct CommentPayload<'a> {
    pub id: &'a str,
    pub target_slug: &'a Option<String>,
    pub target_anchor: &'a Option<String>,
    pub resolved_anchor: &'a Option<String>,
    pub resolved: bool,
    pub git_ref: &'a Option<String>,
    pub quote: &'a Option<String>,
    pub author: &'a Option<String>,
    pub tags: &'a Vec<String>,
    pub status: &'a CommentStatus,
    pub content_html: &'a str,
    pub source_path: &'a Option<String>,
    pub note_slug: &'a str,
}

pub fn list_comments(
    config_path: &Path,
    slug: Option<&str>,
    status: Option<&str>,
    json: bool,
) -> Result<()> {
    let (_config, site_index) = load_or_build_site_index(config_path)?;
    let mut comments: Vec<_> = site_index
        .comments
        .iter()
        .filter(|c| {
            let slug_ok = slug
                .map(|s| c.target_slug.as_deref() == Some(s))
                .unwrap_or(true);
            let status_ok = status
                .map(|st| match st.to_lowercase().as_str() {
                    "open" => c.status == CommentStatus::Open,
                    "resolved" => c.status == CommentStatus::Resolved,
                    _ => true,
                })
                .unwrap_or(true);
            slug_ok && status_ok
        })
        .collect();

    comments.sort_by_key(|c| (&c.target_slug, &c.resolved_anchor, &c.id));

    if json {
        let payload: Vec<_> = comments
            .iter()
            .map(|c| CommentPayload {
                id: &c.id,
                target_slug: &c.target_slug,
                target_anchor: &c.target_anchor,
                resolved_anchor: &c.resolved_anchor,
                resolved: c.resolved,
                git_ref: &c.git_ref,
                quote: &c.quote,
                author: &c.author,
                tags: &c.tags,
                status: &c.status,
                content_html: &c.content_html,
                source_path: &c.source_path,
                note_slug: &c.note_slug,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        for c in comments {
            println!(
                "- {} [{}] -> {}{}{}",
                c.id,
                match c.status {
                    CommentStatus::Open => "open",
                    CommentStatus::Resolved => "resolved",
                },
                c.target_slug.as_deref().unwrap_or("unknown"),
                c.resolved_anchor
                    .as_ref()
                    .map(|a| format!("#{}", a))
                    .unwrap_or_default(),
                if c.resolved { " (resolved)" } else { "" }
            );
            if let Some(q) = &c.quote {
                println!("  quote: {}", q);
            }
            if let Some(auth) = &c.author {
                println!("  author: {}", auth);
            }
        }
    }

    Ok(())
}

pub fn add_comment(
    config_path: &Path,
    target_slug: &str,
    target_anchor: Option<&str>,
    quote: Option<&str>,
    author: Option<&str>,
    tags: Vec<String>,
    status: Option<&str>,
    body: &str,
) -> Result<()> {
    let config = Config::from_file(config_path).context("Failed to load configuration")?;
    let vault_dir = config.vault_dir();
    let comments_dir = vault_dir.join("comments");
    fs::create_dir_all(&comments_dir)
        .with_context(|| format!("Failed to create comments dir {:?}", comments_dir))?;

    let ts = Utc::now().format("%Y%m%d%H%M%S");
    let file_name = format!("{}-{}.md", target_slug, ts);
    let path = comments_dir.join(file_name);

    let status_str = status.unwrap_or("open");
    let frontmatter = format!(
        r#"---
title: Comment on {target_slug}
type: comment
target_slug: {target_slug}
{target_anchor_line}{quote_line}{author_line}status: {status_str}
tags: [{tags}]
---

{body}
"#,
        target_slug = target_slug,
        target_anchor_line = target_anchor
            .map(|a| format!("target_anchor: {}\n", a))
            .unwrap_or_default(),
        quote_line = quote
            .map(|q| format!("quote: \"{}\"\n", escape_yaml(q)))
            .unwrap_or_default(),
        author_line = author
            .map(|a| format!("author: \"{}\"\n", escape_yaml(a)))
            .unwrap_or_default(),
        status_str = status_str,
        tags = tags.join(", "),
        body = body
    );

    fs::write(&path, frontmatter)
        .with_context(|| format!("Failed to write comment file {:?}", path))?;

    println!("Created comment at {}", path.display());
    Ok(())
}

fn escape_yaml(input: &str) -> String {
    input.replace('"', "\\\"")
}
