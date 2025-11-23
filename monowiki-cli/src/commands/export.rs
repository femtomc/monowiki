//! Export section-level content for embeddings/agents.

use crate::{cache::load_or_build_site_index, ExportFormat};
use anyhow::{Context, Result};
use monowiki_core::build_search_index;
use std::fs::File;
use std::io::{stdout, BufWriter, Write};
use std::path::Path;

/// Export section-level chunks as JSON/JSONL.
pub fn export_sections(
    config_path: &Path,
    format: ExportFormat,
    output: Option<&Path>,
    with_links: bool,
    pretty: bool,
) -> Result<()> {
    let (config, site_index) = load_or_build_site_index(config_path)?;
    let base_url = config.normalized_base_url();

    let mut records = Vec::new();
    for note in &site_index.notes {
        if note.is_draft() {
            continue;
        }

        let sections = build_search_index(
            &note.slug,
            &note.title,
            &note.content_html,
            &note.tags,
            note.note_type.as_str(),
            &base_url,
        );

        for entry in sections {
            let slug = slug_from_entry(&entry);
            let outgoing = if with_links {
                site_index.graph.outgoing(&note.slug)
            } else {
                Vec::new()
            };
            let backlinks = if with_links {
                site_index.graph.backlinks(&note.slug)
            } else {
                Vec::new()
            };

            records.push(serde_json::json!({
                "id": entry.id,
                "slug": slug,
                "url": entry.url,
                "title": entry.title,
                "section_title": entry.section_title,
                "content": entry.content,
                "snippet": entry.snippet,
                "tags": entry.tags,
                "type": entry.doc_type,
                "outgoing": if with_links { Some(outgoing) } else { None },
                "backlinks": if with_links { Some(backlinks) } else { None },
            }));
        }
    }

    let writer: Box<dyn Write> = if let Some(path) = output {
        let file = File::create(path)
            .with_context(|| format!("Failed to create output file {:?}", path))?;
        Box::new(BufWriter::new(file))
    } else {
        Box::new(stdout())
    };
    write_records(writer, &records, format, pretty)?;

    Ok(())
}

fn write_records(
    mut writer: Box<dyn Write>,
    records: &[serde_json::Value],
    format: ExportFormat,
    pretty: bool,
) -> Result<()> {
    match format {
        ExportFormat::Jsonl => {
            for rec in records {
                writeln!(writer, "{}", serde_json::to_string(rec)?)?;
            }
        }
        ExportFormat::Json => {
            let json = if pretty {
                serde_json::to_string_pretty(records)?
            } else {
                serde_json::to_string(records)?
            };
            writer.write_all(json.as_bytes())?;
        }
    }
    Ok(())
}

fn slug_from_entry(entry: &monowiki_core::SearchEntry) -> String {
    entry
        .id
        .split('#')
        .next()
        .unwrap_or(&entry.id)
        .to_string()
}
