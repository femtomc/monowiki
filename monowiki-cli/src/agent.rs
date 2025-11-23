//! Shared JSON schema helpers for agent-facing commands and APIs.

use chrono::NaiveDate;
use monowiki_core::{Frontmatter, Note, SearchEntry};
use serde::Serialize;

pub const SCHEMA_VERSION: &str = "2024-11-llm-v1";

/// Standard envelope for machine-consumable responses.
#[derive(Serialize)]
pub struct Envelope<T> {
    pub schema_version: &'static str,
    pub kind: &'static str,
    pub data: T,
}

pub fn envelope<T>(kind: &'static str, data: T) -> Envelope<T> {
    Envelope {
        schema_version: SCHEMA_VERSION,
        kind,
        data,
    }
}

#[derive(Serialize)]
pub struct SearchResult {
    pub id: String,
    pub slug: String,
    pub url: String,
    pub title: String,
    pub section_title: String,
    pub snippet: String,
    pub tags: Vec<String>,
    #[serde(rename = "type")]
    pub note_type: String,
    pub score: f32,
    pub outgoing: Vec<String>,
    pub backlinks: Vec<String>,
}

#[derive(Serialize)]
pub struct SearchData {
    pub query: String,
    pub limit: usize,
    pub total: usize,
    pub results: Vec<SearchResult>,
}

#[derive(Serialize)]
pub struct NoteData {
    pub slug: String,
    pub title: String,
    pub url: String,
    #[serde(rename = "type")]
    pub note_type: String,
    pub tags: Vec<String>,
    pub date: Option<String>,
    pub updated: Option<String>,
    pub frontmatter: Frontmatter,
    pub content_html: String,
    pub toc_html: Option<String>,
    pub raw_body: Option<String>,
    pub preview: Option<String>,
    pub outgoing: Vec<String>,
    pub backlinks: Vec<String>,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub slug: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub note_type: Option<String>,
}

#[derive(Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Serialize)]
pub struct GraphNeighborsData {
    pub root: String,
    pub depth: u8,
    pub direction: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
pub struct GraphPathData {
    pub from: String,
    pub to: String,
    pub path: Option<Vec<String>>,
}

pub fn note_to_payload(note: &Note, base_url: &str, backlinks: Vec<String>) -> NoteData {
    NoteData {
        slug: note.slug.clone(),
        title: note.title.clone(),
        url: note.url_with_base(base_url),
        note_type: note.note_type.as_str().to_string(),
        tags: note.tags.clone(),
        date: format_date(note.date),
        updated: format_date(note.updated),
        frontmatter: note.frontmatter.clone(),
        content_html: note.content_html.clone(),
        toc_html: note.toc_html.clone(),
        raw_body: note.raw_body.clone(),
        preview: note.preview.clone(),
        outgoing: note.outgoing_links.clone(),
        backlinks,
    }
}

pub fn search_entry_slug(entry: &SearchEntry) -> String {
    entry
        .id
        .split('#')
        .next()
        .unwrap_or(&entry.id)
        .to_string()
}

fn format_date(date: Option<NaiveDate>) -> Option<String> {
    date.map(|d| d.format("%Y-%m-%d").to_string())
}
