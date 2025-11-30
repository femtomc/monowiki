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
    // Threading fields
    pub parent_id: &'a Option<String>,
    pub thread_root: &'a Option<String>,
    pub depth: u8,
    pub order: u64,
    pub is_reply: bool,
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

    // Sort by thread_root, then order for threaded display
    comments.sort_by(|a, b| {
        let root_a = a.thread_root.as_deref().unwrap_or(&a.id);
        let root_b = b.thread_root.as_deref().unwrap_or(&b.id);
        match root_a.cmp(root_b) {
            std::cmp::Ordering::Equal => a.order.cmp(&b.order),
            other => other,
        }
    });

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
                parent_id: &c.parent_id,
                thread_root: &c.thread_root,
                depth: c.depth,
                order: c.order,
                is_reply: c.is_reply,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        for c in comments {
            // Indent based on depth for threaded view
            let indent = "  ".repeat(c.depth as usize);
            let reply_marker = if c.is_reply { "↳ " } else { "" };

            println!(
                "{}{}{} [{}] -> {}{}{}",
                indent,
                reply_marker,
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
                if c.resolved { " ✓" } else { "" }
            );
            if let Some(q) = &c.quote {
                println!("{}  quote: {}", indent, q);
            }
            if let Some(auth) = &c.author {
                println!("{}  author: {}", indent, auth);
            }
        }
    }

    Ok(())
}

pub fn add_comment(
    config_path: &Path,
    target_slug: &str,
    target_anchor: Option<&str>,
    reply_to: Option<&str>,
    quote: Option<&str>,
    author: Option<&str>,
    git_ref: Option<&str>,
    tags: Vec<String>,
    status: Option<&str>,
    body: &str,
) -> Result<()> {
    let config = Config::from_file(config_path).context("Failed to load configuration")?;
    let vault_dir = config.vault_dir();
    let comments_dir = vault_dir.join("comments");
    fs::create_dir_all(&comments_dir)
        .with_context(|| format!("Failed to create comments dir {:?}", comments_dir))?;

    // Body: if "-" read stdin
    let body = if body == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read comment body from stdin")?;
        buf
    } else {
        body.to_string()
    };

    let ts = Utc::now().format("%Y%m%d%H%M%S");

    // If replying to a comment, use the parent comment's slug as the base
    let (effective_target, parent_id) = if let Some(parent) = reply_to {
        // When replying, target_slug becomes the parent comment's slug
        // and we record parent_id for threading
        (parent.to_string(), Some(parent.to_string()))
    } else {
        (target_slug.to_string(), None)
    };

    let file_name = format!("{}-{}.md", effective_target, ts);
    let path = comments_dir.join(file_name);

    let status_str = status.unwrap_or("open");
    let author_val = author
        .map(|a| a.to_string())
        .or_else(git_default_author)
        .unwrap_or_default();
    let git_ref_val = git_ref
        .map(|r| r.to_string())
        .or_else(git_head_ref)
        .unwrap_or_default();

    let title = if reply_to.is_some() {
        format!("Reply to {}", effective_target)
    } else {
        format!("Comment on {}", target_slug)
    };

    let frontmatter = format!(
        r#"---
title: {title}
type: comment
target_slug: {effective_target}
{target_anchor_line}{parent_id_line}{quote_line}{author_line}{git_ref_line}status: {status_str}
tags: [{tags}]
---

{body}
"#,
        title = title,
        effective_target = effective_target,
        target_anchor_line = target_anchor
            .map(|a| format!("target_anchor: {}\n", a))
            .unwrap_or_default(),
        parent_id_line = parent_id
            .as_ref()
            .map(|p| format!("parent_id: {}\n", p))
            .unwrap_or_default(),
        quote_line = quote
            .map(|q| format!("quote: \"{}\"\n", escape_yaml(q)))
            .unwrap_or_default(),
        author_line = if author_val.is_empty() {
            String::new()
        } else {
            format!("author: \"{}\"\n", escape_yaml(&author_val))
        },
        git_ref_line = if git_ref_val.is_empty() {
            String::new()
        } else {
            format!("git_ref: {}\n", git_ref_val)
        },
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

fn git_default_author() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn git_head_ref() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.is_empty() {
        None
    } else {
        Some(hash)
    }
}
