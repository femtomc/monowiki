//! Graph queries for neighbors and paths.

use crate::{agent, cache::load_or_build_site_index, GraphDirection};
use anyhow::Result;
use monowiki_core::slugify;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

pub fn graph_neighbors(
    config_path: &Path,
    slug: &str,
    depth: u8,
    direction: GraphDirection,
    json: bool,
) -> Result<()> {
    let (config, site_index) = load_or_build_site_index(config_path)?;
    let base_url = config.normalized_base_url();
    let normalized = normalize_slug(slug);

    if !site_index.graph.outgoing.contains_key(&normalized)
        && !site_index.graph.incoming.contains_key(&normalized)
    {
        anyhow::bail!("Slug '{}' not found in graph", slug);
    }

    let neighbors = crawl_neighbors(&site_index.graph, &normalized, depth, direction);

    if json {
        let nodes: Vec<_> = neighbors
            .iter()
            .map(|slug| {
                let meta = site_index.find_by_slug(slug);
                agent::GraphNode {
                    slug: slug.clone(),
                    title: meta.map(|n| n.title.clone()),
                    url: meta.map(|n| n.url_with_base(&base_url)),
                    tags: meta.map(|n| n.tags.clone()),
                    note_type: meta.map(|n| n.note_type.as_str().to_string()),
                }
            })
            .collect();

        let mut edges = Vec::new();
        for src in &neighbors {
            let outgoing = site_index.graph.outgoing(src);
            for tgt in outgoing {
                if neighbors.contains(&tgt) {
                    edges.push(agent::GraphEdge {
                        source: src.clone(),
                        target: tgt,
                    });
                }
            }
        }

        let payload = agent::envelope(
            "graph.neighbors",
            agent::GraphNeighborsData {
                root: normalized.clone(),
                depth,
                direction: direction_label(direction).to_string(),
                nodes,
                edges,
            },
        );
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("Neighbors of '{}' (depth={}):", normalized, depth);
        for slug in neighbors.iter().filter(|s| *s != &normalized) {
            if let Some(note) = site_index.find_by_slug(slug) {
                println!("- {} ({})", note.title, note.url_with_base(&base_url));
            } else {
                println!("- {}", slug);
            }
        }
    }

    Ok(())
}

pub fn graph_path(
    config_path: &Path,
    from: &str,
    to: &str,
    max_depth: u8,
    json: bool,
) -> Result<()> {
    let (config, site_index) = load_or_build_site_index(config_path)?;
    let base_url = config.normalized_base_url();

    let start = normalize_slug(from);
    let goal = normalize_slug(to);

    let path = shortest_path(&site_index.graph, &start, &goal, max_depth);

    if json {
        let payload = agent::envelope(
            "graph.path",
            agent::GraphPathData {
                from: start.clone(),
                to: goal.clone(),
                path: path.clone(),
            },
        );
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else if let Some(path) = path {
        let rendered: Vec<String> = path
            .iter()
            .map(|slug| {
                site_index
                    .find_by_slug(slug)
                    .map(|n| format!("{} ({})", n.title, n.url_with_base(&base_url)))
                    .unwrap_or_else(|| slug.to_string())
            })
            .collect();
        println!("{}", rendered.join(" -> "));
    } else {
        println!("No path found between '{}' and '{}' (max depth {})", from, to, max_depth);
    }

    Ok(())
}

fn normalize_slug(input: &str) -> String {
    let trimmed = input.trim().trim_matches('/');
    let without_html = trimmed.strip_suffix(".html").unwrap_or(trimmed);
    slugify(without_html)
}

fn direction_label(direction: GraphDirection) -> &'static str {
    match direction {
        GraphDirection::Outgoing => "outgoing",
        GraphDirection::Incoming => "incoming",
        GraphDirection::Both => "both",
    }
}

fn crawl_neighbors(
    graph: &monowiki_core::LinkGraph,
    root: &str,
    depth: u8,
    direction: GraphDirection,
) -> HashSet<String> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut frontier: Vec<String> = vec![root.to_string()];
    visited.insert(root.to_string());

    for _ in 0..depth {
        let mut next = Vec::new();
        for node in frontier {
            let mut neighbors = Vec::new();
            if matches!(direction, GraphDirection::Outgoing | GraphDirection::Both) {
                neighbors.extend(graph.outgoing(&node));
            }
            if matches!(direction, GraphDirection::Incoming | GraphDirection::Both) {
                neighbors.extend(graph.backlinks(&node));
            }

            for n in neighbors {
                if visited.insert(n.clone()) {
                    next.push(n);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }

    visited
}

fn shortest_path(
    graph: &monowiki_core::LinkGraph,
    start: &str,
    goal: &str,
    max_depth: u8,
) -> Option<Vec<String>> {
    if start == goal {
        return Some(vec![start.to_string()]);
    }

    let mut queue = VecDeque::new();
    let mut parents: HashMap<String, String> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();

    queue.push_back((start.to_string(), 0u8));
    visited.insert(start.to_string());

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let mut neighbors = graph.outgoing(&current);
        neighbors.extend(graph.backlinks(&current));

        for n in neighbors {
            if visited.insert(n.clone()) {
                parents.insert(n.clone(), current.clone());
                if n == goal {
                    let mut path = vec![goal.to_string()];
                    let mut cur = goal.to_string();
                    while let Some(parent) = parents.get(&cur) {
                        path.push(parent.clone());
                        cur = parent.clone();
                        if cur == start {
                            break;
                        }
                    }
                    path.push(start.to_string());
                    path.reverse();
                    return Some(path);
                }
                queue.push_back((n, depth + 1));
            }
        }
    }

    None
}
