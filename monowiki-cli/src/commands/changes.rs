//! Change summaries since a git ref.

use crate::cache::load_or_build_site_index;
use anyhow::{anyhow, Context, Result};
use monowiki_core::{
    frontmatter, markdown::MarkdownProcessor, search, slugify, Config, SectionDigest,
};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use similar::{ChangeTag, TextDiff};

const MAX_DIFF_LEN: usize = 4000;

#[derive(Serialize)]
pub struct SectionChange {
    pub section_id: String,
    pub heading: String,
    pub change: String, // added/removed/modified
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>, // optional unified diff
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed_tokens: Option<usize>,
}

#[derive(Serialize)]
pub struct NoteChange {
    pub slug: String,
    pub prev_slug: Option<String>,
    pub path: String,
    pub status: String, // added/modified/deleted
    pub commit_hash: Option<String>,
    pub timestamp: Option<i64>,
    pub last_editor: Option<String>,
    pub sections: Vec<SectionChange>,
}

#[derive(Serialize)]
pub struct ChangesResponse {
    pub since: String,
    pub changes: Vec<NoteChange>,
}

pub fn changes(
    config_path: &Path,
    since: &str,
    json: bool,
    with_sections: bool,
    with_diff: bool,
) -> Result<()> {
    let (config, site_index) = load_or_build_site_index(config_path)?;
    let mut changes = compute_changes(&config, &site_index, since, with_sections, with_diff)?;
    changes.changes.sort_by(|a, b| a.slug.cmp(&b.slug));

    if json {
        let payload = serde_json::to_string_pretty(&changes)?;
        println!("{}", payload);
    } else {
        println!("Changes since {}", since);
        for change in &changes.changes {
            println!("- {} {} ({})", change.status, change.slug, change.path);
            if with_sections && !change.sections.is_empty() {
                for sec in &change.sections {
                    println!("    {}: {}", sec.change, sec.heading);
                }
            }
        }
    }

    Ok(())
}

pub fn compute_changes(
    config: &Config,
    site_index: &monowiki_core::SiteIndex,
    since: &str,
    with_sections: bool,
    with_diff: bool,
) -> Result<ChangesResponse> {
    let include_sections = with_sections || with_diff;
    let git_root = git_root()?;
    let vault_dir = config.vault_dir();
    let vault_rel = vault_dir
        .strip_prefix(&git_root)
        .unwrap_or(&vault_dir)
        .to_path_buf();

    let diff_entries = git_diff_since(since, &vault_rel)?;

    let mut source_map: HashMap<String, &monowiki_core::Note> = HashMap::new();
    for note in &site_index.notes {
        if let Some(path) = &note.source_path {
            source_map.insert(path.clone(), note);
        }
    }

    let mut changes = Vec::new();
    let processor = MarkdownProcessor::new();

    for (status, path) in diff_entries {
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let path_str = path.to_string_lossy().to_string();
        let current_note = source_map.get(&path_str).copied();

        match status.as_str() {
            "A" | "M" => {
                if let Some(note) = current_note {
                    let current_sections = if include_sections {
                        section_snapshots_from_html(note)
                    } else {
                        Vec::new()
                    };
                    let prev_sections = if status == "M" {
                        if include_sections {
                            git_show_file(since, &path)
                                .ok()
                                .and_then(|content| {
                                    section_snapshots_from_markdown(&processor, &content, &path)
                                        .ok()
                                })
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    };

                    let sections = if include_sections {
                        diff_sections(prev_sections, current_sections, with_diff)
                    } else {
                        Vec::new()
                    };

                    let (commit_hash, timestamp, last_editor) =
                        latest_commit_since(since, &path).unwrap_or_default();

                    changes.push(NoteChange {
                        slug: note.slug.clone(),
                        prev_slug: None,
                        path: path_str,
                        status: if status == "A" {
                            "added".into()
                        } else {
                            "modified".into()
                        },
                        commit_hash,
                        timestamp,
                        last_editor,
                        sections,
                    });
                }
            }
            "D" => {
                let prev_content = git_show_file(since, &path).unwrap_or_default();
                let (prev_slug, _) = slug_from_content(&prev_content, &path)
                    .unwrap_or_else(|_| ("".into(), String::new()));
                let prev_sections = if include_sections {
                    section_snapshots_from_markdown(&processor, &prev_content, &path)
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let (commit_hash, timestamp, last_editor) =
                    latest_commit_since(since, &path).unwrap_or_default();

                let sections = if include_sections {
                    diff_sections(prev_sections, Vec::new(), with_diff)
                } else {
                    Vec::new()
                };

                changes.push(NoteChange {
                    slug: prev_slug.clone(),
                    prev_slug: Some(prev_slug),
                    path: path_str,
                    status: "deleted".into(),
                    commit_hash,
                    timestamp,
                    last_editor,
                    sections,
                });
            }
            _ => {}
        }
    }

    Ok(ChangesResponse {
        since: since.to_string(),
        changes,
    })
}

struct SectionSnapshot {
    digest: SectionDigest,
    content: String,
}

fn section_snapshots_from_html(note: &monowiki_core::Note) -> Vec<SectionSnapshot> {
    let entries = search::build_search_index(
        &note.slug,
        &note.title,
        &note.content_html,
        &note.tags,
        note.note_type.as_str(),
        "/",
    );
    entries
        .into_iter()
        .map(|entry| SectionSnapshot {
            digest: SectionDigest {
                section_id: entry.section_id,
                heading: entry.section_title,
                hash: entry.section_hash,
                anchor_id: entry.id.split('#').nth(1).map(|s| s.to_string()),
            },
            content: entry.content,
        })
        .collect()
}

fn section_snapshots_from_markdown(
    processor: &MarkdownProcessor,
    markdown: &str,
    path: &Path,
) -> Result<Vec<SectionSnapshot>> {
    let (slug, title) = slug_from_content(markdown, path)?;
    let html = processor.convert_simple(markdown);
    let entries = search::build_search_index(&slug, &title, &html, &[], "", "/");
    Ok(entries
        .into_iter()
        .map(|entry| SectionSnapshot {
            digest: SectionDigest {
                section_id: entry.section_id,
                heading: entry.section_title,
                hash: entry.section_hash,
                anchor_id: entry.id.split('#').nth(1).map(|s| s.to_string()),
            },
            content: entry.content,
        })
        .collect())
}

fn slug_from_content(content: &str, path: &Path) -> Result<(String, String)> {
    let (fm, _body) = frontmatter::parse_frontmatter(content)?;
    let slug = if let Some(s) = fm.slug {
        s
    } else {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(slugify)
            .unwrap_or_else(|| slugify(&fm.title))
    };
    let title = if fm.title.trim().is_empty() {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    } else {
        fm.title
    };
    Ok((slug, title))
}

fn diff_sections(
    prev: Vec<SectionSnapshot>,
    current: Vec<SectionSnapshot>,
    include_diff: bool,
) -> Vec<SectionChange> {
    let mut changes = Vec::new();
    let mut prev_map: HashMap<String, SectionSnapshot> = HashMap::new();
    for s in prev {
        prev_map.insert(s.digest.section_id.clone(), s);
    }

    for curr in &current {
        match prev_map.remove(&curr.digest.section_id) {
            Some(prev_sec) if prev_sec.digest.hash != curr.digest.hash => {
                let (diff, added_tokens, removed_tokens) = if include_diff {
                    compute_section_diff(&prev_sec.content, &curr.content)
                } else {
                    (None, None, None)
                };
                changes.push(SectionChange {
                    section_id: curr.digest.section_id.clone(),
                    heading: curr.digest.heading.clone(),
                    change: "modified".into(),
                    diff,
                    added_tokens,
                    removed_tokens,
                });
            }
            None => {
                let (diff, added_tokens, removed_tokens) = if include_diff {
                    compute_section_diff("", &curr.content)
                } else {
                    (None, None, None)
                };
                changes.push(SectionChange {
                    section_id: curr.digest.section_id.clone(),
                    heading: curr.digest.heading.clone(),
                    change: "added".into(),
                    diff,
                    added_tokens,
                    removed_tokens,
                });
            }
            _ => {}
        }
    }

    for prev_sec in prev_map.values() {
        let (diff, added_tokens, removed_tokens) = if include_diff {
            compute_section_diff(&prev_sec.content, "")
        } else {
            (None, None, None)
        };
        changes.push(SectionChange {
            section_id: prev_sec.digest.section_id.clone(),
            heading: prev_sec.digest.heading.clone(),
            change: "removed".into(),
            diff,
            added_tokens,
            removed_tokens,
        });
    }

    changes
}

fn compute_section_diff(
    previous: &str,
    current: &str,
) -> (Option<String>, Option<usize>, Option<usize>) {
    let diff = TextDiff::from_words(previous, current);
    let mut added_tokens = 0usize;
    let mut removed_tokens = 0usize;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                removed_tokens += change.value().split_whitespace().count();
            }
            ChangeTag::Insert => {
                added_tokens += change.value().split_whitespace().count();
            }
            _ => {}
        }
    }

    let mut unified = diff
        .unified_diff()
        .context_radius(2)
        .header("previous", "current")
        .to_string();

    if unified.len() > MAX_DIFF_LEN {
        unified.truncate(MAX_DIFF_LEN);
        unified.push_str("\n...diff truncated...");
    }

    (
        Some(unified),
        Some(added_tokens),
        Some(removed_tokens),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: &str, heading: &str, content: &str, hash: &str) -> SectionSnapshot {
        SectionSnapshot {
            digest: SectionDigest {
                section_id: id.to_string(),
                heading: heading.to_string(),
                hash: hash.into(),
                anchor_id: None,
            },
            content: content.to_string(),
        }
    }

    #[test]
    fn diff_sections_includes_diffs_when_enabled() {
        let prev = vec![snap("s1", "Intro", "hello world", "h1")];
        let cur = vec![snap("s1", "Intro", "hello brave world", "h2")];

        let changes = diff_sections(prev, cur, true);
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert_eq!(change.change, "modified");
        let diff_text = change.diff.as_ref().unwrap();
        assert!(diff_text.contains('+'));
        assert_eq!(change.added_tokens, Some(1));
        assert_eq!(change.removed_tokens, Some(0));
    }

    #[test]
    fn diff_sections_skips_diffs_when_disabled() {
        let prev = vec![snap("s1", "Intro", "hello world", "h1")];
        let cur = vec![snap("s1", "Intro", "hello brave world", "h2")];

        let changes = diff_sections(prev, cur, false);
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert_eq!(change.change, "modified");
        assert!(change.diff.is_none());
        assert!(change.added_tokens.is_none());
        assert!(change.removed_tokens.is_none());
    }

    #[test]
    fn compute_diff_truncates_large_output() {
        let prev = "a ".repeat(3000);
        let curr = "b ".repeat(3000);

        let (diff, added, removed) = compute_section_diff(&prev, &curr);
        let diff_text = diff.expect("diff");
        assert!(diff_text.contains("...diff truncated..."));
        assert!(diff_text.len() >= MAX_DIFF_LEN); // truncated marker appended
        assert_eq!(added, Some(3000));
        assert_eq!(removed, Some(3000));
    }
}

fn git_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("Failed to run git rev-parse")?;
    if !output.status.success() {
        return Err(anyhow!("Not a git repository"));
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

fn git_diff_since(since: &str, vault_rel: &Path) -> Result<Vec<(String, PathBuf)>> {
    let vault_str = vault_rel.to_string_lossy();
    let output = Command::new("git")
        .args(["diff", "--name-status", since, "--", &vault_str])
        .output()
        .context("Failed to run git diff")?;
    if !output.status.success() {
        return Err(anyhow!("git diff failed"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(status), Some(path)) = (parts.next(), parts.next()) {
            entries.push((status.to_string(), PathBuf::from(path)));
        }
    }
    Ok(entries)
}

fn git_show_file(since: &str, path: &Path) -> Result<String> {
    let spec = format!("{}:{}", since, path.to_string_lossy());
    let output = Command::new("git")
        .args(["show", &spec])
        .output()
        .context("Failed to run git show")?;
    if !output.status.success() {
        return Err(anyhow!("git show failed for {}", spec));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn latest_commit_since(
    since: &str,
    path: &Path,
) -> Result<(Option<String>, Option<i64>, Option<String>)> {
    let range = format!("{}..HEAD", since);
    let output = Command::new("git")
        .args([
            "log",
            "-1",
            "--format=%H|%ct|%an",
            &range,
            "--",
            &path.to_string_lossy(),
        ])
        .output()
        .context("Failed to run git log")?;
    if !output.status.success() {
        return latest_commit(&path.to_string_lossy());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return latest_commit(&path.to_string_lossy());
    }
    let mut parts = stdout.trim().split('|');
    let hash = parts.next().map(|s| s.to_string());
    let ts = parts.next().and_then(|s| s.parse::<i64>().ok());
    let author = parts.next().map(|s| s.to_string());
    Ok((hash, ts, author))
}

fn latest_commit(path: &str) -> Result<(Option<String>, Option<i64>, Option<String>)> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%H|%ct|%an", "--", path])
        .output()
        .context("Failed to run git log fallback")?;
    if !output.status.success() {
        return Ok((None, None, None));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return Ok((None, None, None));
    }
    let mut parts = stdout.trim().split('|');
    let hash = parts.next().map(|s| s.to_string());
    let ts = parts.next().and_then(|s| s.parse::<i64>().ok());
    let author = parts.next().map(|s| s.to_string());
    Ok((hash, ts, author))
}
