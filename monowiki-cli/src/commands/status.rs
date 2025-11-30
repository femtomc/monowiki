//! Aggregated status surface for agents (changes + comments).

use crate::cache::load_or_build_site_index;
use crate::commands::changes::ChangesResponse;
use crate::commands::compute_changes;
use anyhow::Result;
use monowiki_core::CommentStatus;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct StatusResponse {
    pub changes: ChangesResponse,
    pub comments: Vec<monowiki_core::Comment>,
}

pub fn status(
    config_path: &Path,
    since: &str,
    comment_status: Option<String>,
    with_sections: bool,
    json: bool,
) -> Result<()> {
    let (config, site_index) = load_or_build_site_index(config_path)?;
    let changes = compute_changes(&config, &site_index, since, with_sections, false)?;

    let mut comments: Vec<_> = site_index
        .comments
        .into_iter()
        .filter(
            |c| match comment_status.as_deref().map(|s| s.to_lowercase()) {
                Some(ref s) if s == "open" => c.status == CommentStatus::Open,
                Some(ref s) if s == "resolved" => c.status == CommentStatus::Resolved,
                _ => true,
            },
        )
        .collect();
    comments.sort_by(|a, b| {
        (&a.target_slug, &a.resolved_anchor, &a.id).cmp(&(
            &b.target_slug,
            &b.resolved_anchor,
            &b.id,
        ))
    });

    if json {
        let payload = StatusResponse { changes, comments };
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("Status since {}", since);
        println!("Changes:");
        for ch in &changes.changes {
            println!("- {} {} ({})", ch.status, ch.slug, ch.path);
            if with_sections && !ch.sections.is_empty() {
                for sec in &ch.sections {
                    println!("    {} {}", sec.change, sec.heading);
                }
            }
        }
        println!("\nComments:");
        for c in &comments {
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
        }
    }

    Ok(())
}
