///! Search command implementation
use crate::agent;
use anyhow::{Context, Result};
use monowiki_core::{Config, SearchEntry};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub limit: usize,
    pub json: bool,
    pub types: Vec<String>,
    pub tags: Vec<String>,
    pub with_links: bool,
}

/// Search the site index
pub fn search_site(config_path: &Path, query: &str, opts: SearchOptions) -> Result<()> {
    let config = Config::from_file(config_path).context("Failed to load configuration")?;
    let index_path = config.output_dir().join("index.json");

    if !index_path.exists() {
        eprintln!("❌ Search index not found. Run `monowiki build` first.");
        std::process::exit(1);
    }

    // Load search index
    let index_json = fs::read_to_string(&index_path).context("Failed to read search index")?;
    let entries: Vec<SearchEntry> =
        serde_json::from_str(&index_json).context("Failed to parse search index")?;

    let graph = if opts.with_links {
        load_graph(&config)?
    } else {
        GraphInfo::default()
    };

    let results = perform_search(&entries, query, &opts);

    if results.is_empty() {
        println!("No results found for '{}'", query);
        return Ok(());
    }

    if opts.json {
        let mut json_results = Vec::new();
        for (entry, score) in results.iter().take(opts.limit) {
            let slug = agent::search_entry_slug(entry);
            let outgoing = graph.outgoing.get(&slug).cloned().unwrap_or_default();
            let backlinks = graph.incoming.get(&slug).cloned().unwrap_or_default();

            json_results.push(agent::SearchResult {
                id: entry.id.clone(),
                slug,
                url: entry.url.clone(),
                title: entry.title.clone(),
                section_title: entry.section_title.clone(),
                snippet: entry.snippet.clone(),
                tags: entry.tags.clone(),
                note_type: entry.doc_type.clone(),
                score: *score,
                outgoing: if opts.with_links {
                    outgoing
                } else {
                    Vec::new()
                },
                backlinks: if opts.with_links {
                    backlinks
                } else {
                    Vec::new()
                },
            });
        }

        let payload = agent::envelope(
            "search.results",
            agent::SearchData {
                query: query.to_string(),
                limit: opts.limit,
                total: results.len(),
                results: json_results,
            },
        );

        let json = serde_json::to_string_pretty(&payload)?;
        println!("{json}");
    } else {
        println!("\n🔍 Found {} results for '{}':\n", results.len(), query);

        for (entry, _score) in results.iter().take(opts.limit) {
            print_search_result(entry);
        }

        if results.len() > opts.limit {
            println!("\n  ... and {} more results", results.len() - opts.limit);
        }
    }

    Ok(())
}

fn print_search_result(entry: &SearchEntry) {
    // Format:
    // [essay] Why MonoWiki Uses Rust → Performance
    // /rust-rewrite.html#performance
    // Single native binary (no external runtime) Lower memory usage...
    //

    let section_info = if entry.section_title.is_empty() {
        entry.title.clone()
    } else {
        format!("{} → {}", entry.title, entry.section_title)
    };

    println!("[{}] {}", entry.doc_type, section_info);
    println!("  {}", entry.url);
    println!("  {}", entry.snippet);
    println!();
}

/// Perform scored search with optional type/tag filters
pub fn perform_search<'a>(
    entries: &'a [SearchEntry],
    query: &str,
    opts: &SearchOptions,
) -> Vec<(&'a SearchEntry, f32)> {
    // Perform simple text search (case-insensitive)
    let query_lower = query.to_lowercase();
    let type_filter: HashSet<String> = opts.types.iter().map(|t| t.to_lowercase()).collect();
    let tag_filter: HashSet<String> = opts.tags.iter().map(|t| t.to_lowercase()).collect();

    let mut results: Vec<(&SearchEntry, f32)> = entries
        .iter()
        .filter_map(|entry| {
            if !type_filter.is_empty() && !type_filter.contains(&entry.doc_type.to_lowercase()) {
                return None;
            }

            if !tag_filter.is_empty()
                && !entry
                    .tags
                    .iter()
                    .any(|t| tag_filter.contains(&t.to_lowercase()))
            {
                return None;
            }

            let mut score = 0.0;

            // Title match (highest weight)
            if entry.title.to_lowercase().contains(&query_lower) {
                score += 10.0;
            }

            // Section title match (medium weight)
            if entry.section_title.to_lowercase().contains(&query_lower) {
                score += 5.0;
            }

            // Content match (lowest weight)
            if entry.content.to_lowercase().contains(&query_lower) {
                score += 1.0;
            }

            // Tags match
            if entry
                .tags
                .iter()
                .any(|t| t.to_lowercase().contains(&query_lower))
            {
                score += 3.0;
            }

            if score > 0.0 {
                Some((entry, score))
            } else {
                None
            }
        })
        .collect();

    // Sort by score (descending)
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results
}

#[derive(Default)]
struct GraphInfo {
    outgoing: HashMap<String, Vec<String>>,
    incoming: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GraphEdge {
    source: String,
    target: String,
}

#[derive(Debug, Deserialize)]
struct GraphJson {
    edges: Vec<GraphEdge>,
}

fn load_graph(config: &Config) -> Result<GraphInfo> {
    let graph_path = config.output_dir().join("graph.json");
    if !graph_path.exists() {
        return Ok(GraphInfo::default());
    }

    let graph_str = fs::read_to_string(&graph_path).context("Failed to read graph.json")?;
    let parsed: GraphJson =
        serde_json::from_str(&graph_str).context("Failed to parse graph.json")?;

    let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();
    let mut incoming: HashMap<String, Vec<String>> = HashMap::new();

    for edge in parsed.edges {
        outgoing
            .entry(edge.source.clone())
            .or_default()
            .push(edge.target.clone());
        incoming
            .entry(edge.target.clone())
            .or_default()
            .push(edge.source.clone());
    }

    Ok(GraphInfo { outgoing, incoming })
}
