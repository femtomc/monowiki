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

#[derive(Serialize)]
pub struct SectionChange {
    pub section_id: String,
    pub heading: String,
    pub change: String, // added/removed/modified
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

pub fn changes(config_path: &Path, since: &str, json: bool, with_sections: bool) -> Result<()> {
    let (config, site_index) = load_or_build_site_index(config_path)?;
    let mut changes = compute_changes(&config, &site_index, since, with_sections)?;
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
) -> Result<ChangesResponse> {
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
                    let current_sections = section_digests_from_note(note, with_sections);
                    let prev_sections = if status == "M" {
                        git_show_file(since, &path)
                            .ok()
                            .and_then(|content| {
                                compute_sections_from_markdown(&processor, &content, &path).ok()
                            })
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    let sections = if with_sections {
                        diff_sections(prev_sections, current_sections)
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
                    .unwrap_or_else(|_| ("".into(), "".into()));
                let prev_sections = if with_sections {
                    compute_sections_from_markdown(&processor, &prev_content, &path)
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let (commit_hash, timestamp, last_editor) =
                    latest_commit_since(since, &path).unwrap_or_default();

                let sections = if with_sections {
                    prev_sections
                        .into_iter()
                        .map(|s| SectionChange {
                            section_id: s.section_id,
                            heading: s.heading,
                            change: "removed".into(),
                        })
                        .collect()
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

fn section_digests_from_note(
    note: &monowiki_core::Note,
    with_sections: bool,
) -> Vec<SectionDigest> {
    if !with_sections {
        return Vec::new();
    }
    search::section_digests_from_html(&note.slug, &note.title, &note.content_html)
}

fn compute_sections_from_markdown(
    processor: &MarkdownProcessor,
    markdown: &str,
    path: &Path,
) -> Result<Vec<SectionDigest>> {
    let (slug, title) = slug_from_content(markdown, path)?;
    let html = processor.convert_simple(markdown);
    Ok(search::section_digests_from_html(&slug, &title, &html))
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

fn diff_sections(prev: Vec<SectionDigest>, current: Vec<SectionDigest>) -> Vec<SectionChange> {
    let mut changes = Vec::new();
    let mut prev_map: HashMap<String, SectionDigest> = HashMap::new();
    for s in prev {
        prev_map.insert(s.section_id.clone(), s);
    }

    for curr in &current {
        match prev_map.get(&curr.section_id) {
            Some(prev_sec) if prev_sec.hash != curr.hash => {
                changes.push(SectionChange {
                    section_id: curr.section_id.clone(),
                    heading: curr.heading.clone(),
                    change: "modified".into(),
                });
            }
            None => {
                changes.push(SectionChange {
                    section_id: curr.section_id.clone(),
                    heading: curr.heading.clone(),
                    change: "added".into(),
                });
            }
            _ => {}
        }
    }

    for prev_sec in prev_map.values() {
        if !current.iter().any(|c| c.section_id == prev_sec.section_id) {
            changes.push(SectionChange {
                section_id: prev_sec.section_id.clone(),
                heading: prev_sec.heading.clone(),
                change: "removed".into(),
            });
        }
    }

    changes
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
