//! Dev server command implementation with JSON APIs.

use super::build::build_site_with_index;
use super::search::{perform_search, SearchOptions};
use crate::{agent, GraphDirection};
use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use monowiki_core::{build_search_index, slugify, CommentStatus, Config, SearchEntry};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::{mpsc, RwLock};

#[derive(Clone)]
struct AppState {
    output_dir: PathBuf,
    data: Arc<RwLock<SiteData>>,
}

struct SiteData {
    config: Config,
    site_index: monowiki_core::SiteIndex,
    search_entries: Vec<SearchEntry>,
    base_url: String,
}

/// Start development server with file watching
pub async fn dev_server(config_path: &Path, port: u16) -> Result<()> {
    // Initial build + in-memory index
    let site_data = build_site_data(config_path).context("Failed to build site")?;
    let output_dir = site_data.config.output_dir();
    let vault_dir = site_data.config.vault_dir();
    let config_path_buf = config_path.to_path_buf();
    let shared_data = Arc::new(RwLock::new(site_data));

    tracing::info!("Starting dev server on http://localhost:{}", port);
    println!("\n🚀 Serving at http://localhost:{}", port);
    println!("   Press Ctrl+C to stop\n");

    // Set up file watching for live rebuilds
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut _watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        notify::Config::default(),
    )
    .context("Failed to initialize file watcher")?;

    _watcher
        .watch(&vault_dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to watch {:?}", vault_dir))?;

    tokio::spawn({
        let data_handle = shared_data.clone();
        async move {
            while let Some(event) = rx.recv().await {
                match event {
                    Ok(_ev) => {
                        // Debounce a bit by draining pending events
                        while rx.try_recv().is_ok() {}
                        tracing::info!("Change detected, rebuilding site...");
                        let res = tokio::task::spawn_blocking({
                            let config_path = config_path_buf.clone();
                            move || build_site_with_index(&config_path)
                        })
                        .await;

                        match res {
                            Ok(Ok((config, site_index))) => {
                                let base_url = config.normalized_base_url();
                                let search_entries = compute_search_entries(&site_index, &base_url);

                                let mut data = data_handle.write().await;
                                *data = SiteData {
                                    config,
                                    site_index,
                                    search_entries,
                                    base_url,
                                };
                                tracing::info!("Rebuild complete");
                            }
                            Ok(Err(e)) => tracing::error!("Rebuild failed: {:?}", e),
                            Err(e) => tracing::error!("Rebuild task panicked: {}", e),
                        }
                    }
                    Err(err) => tracing::warn!("Watcher error: {}", err),
                }
            }
        }
    });

    let state = AppState {
        output_dir: output_dir.clone(),
        data: shared_data.clone(),
    };

    // Build router
    let app = Router::new()
        .route("/api/search", get(api_search))
        .route("/api/note/{slug}", get(api_note))
        .route("/api/graph/{slug}", get(api_graph_neighbors))
        .route("/api/graph/path", get(api_graph_path))
        .route("/api/status", get(api_status))
        .route("/api/comments", get(api_comments))
        .route("/api/changes", get(api_changes))
        .route("/{*path}", get(serve_with_404))
        .route("/", get(serve_index))
        .fallback(serve_404)
        .with_state(state);

    // Start server
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    axum::serve(listener, app).await.context("Server error")?;

    Ok(())
}

/// Serve index.html for root path
async fn serve_index(State(state): State<AppState>) -> Response {
    let index_path = state.output_dir.join("index.html");
    match fs::read_to_string(&index_path).await {
        Ok(content) => Html(content).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Index not found").into_response(),
    }
}

/// Serve files with custom 404 handling
async fn serve_with_404(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let file_path = state.output_dir.join(path);

    match fs::read(&file_path).await {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", content_type_for_path(path))
            .body(Body::from(content))
            .unwrap(),
        Err(_) => serve_404_inner(state).await,
    }
}

/// Serve custom 404 page
async fn serve_404(State(state): State<AppState>) -> Response {
    serve_404_inner(state).await
}

async fn serve_404_inner(state: AppState) -> Response {
    let not_found_path = state.output_dir.join("404.html");

    match fs::read_to_string(&not_found_path).await {
        Ok(content) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "text/html; charset=utf-8")
            .body(Body::from(content))
            .unwrap(),
        Err(_) => {
            // Fallback if 404.html doesn't exist
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))
                .unwrap()
        }
    }
}

// ---- API handlers ----

#[derive(Deserialize)]
struct SearchParams {
    q: Option<String>,
    limit: Option<usize>,
    types: Option<String>,
    tags: Option<String>,
}

async fn api_search(State(state): State<AppState>, Query(params): Query<SearchParams>) -> Response {
    let query = params.q.unwrap_or_default();
    if query.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Missing query ?q=").into_response();
    }

    let limit = params.limit.unwrap_or(10);
    let types = split_csv(params.types);
    let tags = split_csv(params.tags);

    let opts = SearchOptions {
        limit,
        json: true,
        types,
        tags,
        with_links: true,
    };

    let data = state.data.read().await;
    let results = perform_search(&data.search_entries, &query, &opts);
    let total = results.len();

    let mut payload_results = Vec::new();
    for (entry, score) in results.into_iter().take(limit) {
        let slug = agent::search_entry_slug(entry);
        let outgoing = data.site_index.graph.outgoing(&slug);
        let backlinks = data.site_index.graph.backlinks(&slug);

        payload_results.push(agent::SearchResult {
            id: entry.id.clone(),
            slug,
            url: entry.url.clone(),
            title: entry.title.clone(),
            section_title: entry.section_title.clone(),
            snippet: entry.snippet.clone(),
            tags: entry.tags.clone(),
            note_type: entry.doc_type.clone(),
            score,
            outgoing,
            backlinks,
        });
    }

    let payload = agent::envelope(
        "search.results",
        agent::SearchData {
            query: query.clone(),
            limit,
            total,
            results: payload_results,
        },
    );

    Json(payload).into_response()
}

#[derive(Deserialize)]
struct ChangesParams {
    since: Option<String>,
    with_sections: Option<bool>,
}

#[derive(Deserialize)]
struct CommentsParams {
    slug: Option<String>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct StatusParams {
    since: Option<String>,
    comment_status: Option<String>,
    with_sections: Option<bool>,
}

async fn api_changes(
    State(state): State<AppState>,
    Query(params): Query<ChangesParams>,
) -> Response {
    let since = params.since.unwrap_or_else(|| "HEAD~1".to_string());
    let with_sections = params.with_sections.unwrap_or(false);
    let data = state.data.read().await;
    let site_index = data.site_index.clone();
    let config = data.config.clone();

    // Run blocking git operations off the main executor
    let result = tokio::task::spawn_blocking(move || {
        crate::commands::compute_changes(&config, &site_index, &since, with_sections)
    })
    .await;

    match result {
        Ok(Ok(changes)) => {
            let json = serde_json::to_string(&changes).unwrap_or_else(|_| "{}".into());
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                json,
            )
                .into_response()
        }
        Ok(Err(err)) => (
            StatusCode::BAD_REQUEST,
            format!("Failed to compute changes: {}", err),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", err),
        )
            .into_response(),
    }
}

async fn api_comments(
    State(state): State<AppState>,
    Query(params): Query<CommentsParams>,
) -> Response {
    let data = state.data.read().await;
    let mut comments_vec: Vec<_> = data
        .site_index
        .comments
        .iter()
        .filter(|c| {
            let slug_ok = params
                .slug
                .as_ref()
                .map(|s| c.target_slug.as_deref() == Some(s.as_str()))
                .unwrap_or(true);
            let status_ok = params
                .status
                .as_ref()
                .map(|st| match st.to_lowercase().as_str() {
                    "open" => c.status == CommentStatus::Open,
                    "resolved" => c.status == CommentStatus::Resolved,
                    _ => true,
                })
                .unwrap_or(true);
            slug_ok && status_ok
        })
        .collect();
    comments_vec.sort_by(|a, b| {
        (&a.target_slug, &a.resolved_anchor, &a.id).cmp(&(
            &b.target_slug,
            &b.resolved_anchor,
            &b.id,
        ))
    });

    let json = serde_json::to_string(&comments_vec).unwrap_or_else(|_| "[]".into());
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        json,
    )
        .into_response()
}

async fn api_status(State(state): State<AppState>, Query(params): Query<StatusParams>) -> Response {
    let since = params.since.unwrap_or_else(|| "HEAD~1".to_string());
    let with_sections = params.with_sections.unwrap_or(false);
    let comment_status = params.comment_status.clone();
    let data = state.data.read().await;
    let site_index = data.site_index.clone();
    let config = data.config.clone();

    let result = tokio::task::spawn_blocking(move || {
        let changes =
            crate::commands::compute_changes(&config, &site_index, &since, with_sections)?;

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

        let payload = serde_json::json!({
            "changes": changes,
            "comments": comments,
        });
        Ok::<_, anyhow::Error>(payload)
    })
    .await;

    match result {
        Ok(Ok(payload)) => {
            let json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into());
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                json,
            )
                .into_response()
        }
        Ok(Err(err)) => (
            StatusCode::BAD_REQUEST,
            format!("Failed to compute status: {}", err),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Task join error: {}", err),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct GraphParams {
    depth: Option<u8>,
    direction: Option<String>,
}

async fn api_graph_neighbors(
    AxumPath(slug): AxumPath<String>,
    State(state): State<AppState>,
    Query(params): Query<GraphParams>,
) -> Response {
    let direction = match params
        .direction
        .as_deref()
        .map(|d| d.to_lowercase())
        .as_deref()
    {
        Some("incoming") => GraphDirection::Incoming,
        Some("outgoing") => GraphDirection::Outgoing,
        _ => GraphDirection::Both,
    };
    let depth = params.depth.unwrap_or(1);

    let data = state.data.read().await;
    let normalized = normalize_slugish(&slug);

    if !data.site_index.graph.outgoing.contains_key(&normalized)
        && !data.site_index.graph.incoming.contains_key(&normalized)
    {
        return (StatusCode::NOT_FOUND, "Slug not in graph").into_response();
    }

    let neighbors = crawl_neighbors(&data.site_index.graph, &normalized, depth, direction);

    let nodes: Vec<_> = neighbors
        .iter()
        .map(|slug| {
            let meta = data.site_index.find_by_slug(slug);
            agent::GraphNode {
                slug: slug.clone(),
                title: meta.map(|n| n.title.clone()),
                url: meta.map(|n| n.url_with_base(&data.base_url)),
                tags: meta.map(|n| n.tags.clone()),
                note_type: meta.map(|n| n.note_type.as_str().to_string()),
            }
        })
        .collect();

    let mut edges = Vec::new();
    for src in &neighbors {
        let outgoing = data.site_index.graph.outgoing(src);
        for tgt in outgoing {
            if neighbors.contains(&tgt) {
                edges.push(agent::GraphEdge {
                    source: src.clone(),
                    target: tgt,
                });
            }
        }
    }

    Json(agent::envelope(
        "graph.neighbors",
        agent::GraphNeighborsData {
            root: normalized,
            depth,
            direction: direction_label(direction).to_string(),
            nodes,
            edges,
        },
    ))
    .into_response()
}

#[derive(Deserialize)]
struct GraphPathParams {
    from: Option<String>,
    to: Option<String>,
    max_depth: Option<u8>,
}

async fn api_graph_path(
    State(state): State<AppState>,
    Query(params): Query<GraphPathParams>,
) -> Response {
    let from = if let Some(f) = params.from.clone() {
        f
    } else {
        return (StatusCode::BAD_REQUEST, "Missing from").into_response();
    };
    let to = if let Some(t) = params.to.clone() {
        t
    } else {
        return (StatusCode::BAD_REQUEST, "Missing to").into_response();
    };

    let max_depth = params.max_depth.unwrap_or(5);
    let data = state.data.read().await;
    let start = normalize_slugish(&from);
    let goal = normalize_slugish(&to);

    let path = shortest_path(&data.site_index.graph, &start, &goal, max_depth);
    Json(agent::envelope(
        "graph.path",
        agent::GraphPathData {
            from: start,
            to: goal,
            path,
        },
    ))
    .into_response()
}

async fn api_note(AxumPath(slug): AxumPath<String>, State(state): State<AppState>) -> Response {
    let data = state.data.read().await;
    let normalized = normalize_slugish(&slug);

    let note = data
        .site_index
        .find_by_slug(&normalized)
        .or_else(|| data.site_index.find_by_alias(&normalized))
        .or_else(|| data.site_index.find_by_permalink(&slug));

    if let Some(note) = note {
        let backlinks = data.site_index.graph.backlinks(&note.slug);
        let payload = agent::envelope(
            "note.full",
            agent::note_to_payload(note, &data.base_url, backlinks),
        );

        Json(payload).into_response()
    } else {
        (StatusCode::NOT_FOUND, "Note not found").into_response()
    }
}

// ---- helpers ----

fn build_site_data(config_path: &Path) -> Result<SiteData> {
    let (config, site_index) = build_site_with_index(config_path)?;
    let base_url = config.normalized_base_url();
    let search_entries = compute_search_entries(&site_index, &base_url);

    Ok(SiteData {
        config,
        site_index,
        search_entries,
        base_url,
    })
}

fn compute_search_entries(
    site_index: &monowiki_core::SiteIndex,
    base_url: &str,
) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for note in &site_index.notes {
        if note.is_draft() || note.note_type == monowiki_core::NoteType::Comment {
            continue;
        }

        let mut note_entries = build_search_index(
            &note.slug,
            &note.title,
            &note.content_html,
            &note.tags,
            note.note_type.as_str(),
            base_url,
        );
        entries.append(&mut note_entries);
    }
    entries
}

fn content_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn normalize_slugish(s: &str) -> String {
    let trimmed = s.trim().trim_matches('/');
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

fn split_csv(input: Option<String>) -> Vec<String> {
    input
        .map(|s| {
            s.split(',')
                .filter_map(|p| {
                    let trimmed = p.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    }
                })
                .collect()
        })
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use monowiki_core::config::Config;
    use monowiki_core::{Frontmatter, LinkGraph, Note, NoteType, SiteIndex};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn sample_state() -> AppState {
        let tmp = tempdir().unwrap();
        let config_path = tmp.path().join("monowiki.yml");
        fs::write(
            &config_path,
            r#"
site:
  title: "Test"
  author: "Tester"
  description: "Desc"
  url: "https://example.com"
paths:
  vault: "vault"
  output: "docs"
  templates: null
  theme: null
base_url: "/"
enable_backlinks: true
"#,
        )
        .unwrap();

        let config = Config::from_file(&config_path).unwrap();

        let mut graph = LinkGraph::new();
        graph.add_link("note-a", "note-b");
        graph.add_link("note-b", "note-a");

        let note_a = Note {
            slug: "note-a".into(),
            title: "Note A".into(),
            content_html: "<h1 id=\"intro\">Intro</h1><p>Rust content</p>".into(),
            frontmatter: Frontmatter::default(),
            note_type: NoteType::Essay,
            tags: vec!["rust".into()],
            date: None,
            updated: None,
            aliases: vec![],
            permalink: None,
            outgoing_links: vec!["note-b".into()],
            preview: Some("Rust content".into()),
            toc_html: None,
            raw_body: Some("# Intro\nRust content".into()),
            source_path: None,
        };

        let note_b = Note {
            slug: "note-b".into(),
            title: "Note B".into(),
            content_html: "<p>Memory</p>".into(),
            frontmatter: Frontmatter::default(),
            note_type: NoteType::Essay,
            tags: vec!["memory".into()],
            date: None,
            updated: None,
            aliases: vec![],
            permalink: None,
            outgoing_links: vec!["note-a".into()],
            preview: Some("Memory".into()),
            toc_html: None,
            raw_body: Some("Memory".into()),
            source_path: None,
        };

        let mut site_index = SiteIndex {
            notes: vec![note_a.clone(), note_b.clone()],
            graph,
            comments: Vec::new(),
            diagnostics: Vec::new(),
        };

        let base_url = config.normalized_base_url();
        let mut entries = build_search_index(
            &note_a.slug,
            &note_a.title,
            &note_a.content_html,
            &note_a.tags,
            note_a.note_type.as_str(),
            &base_url,
        );
        entries.extend(build_search_index(
            &note_b.slug,
            &note_b.title,
            &note_b.content_html,
            &note_b.tags,
            note_b.note_type.as_str(),
            &base_url,
        ));

        // Ensure raw bodies stored
        site_index.notes[0].raw_body = note_a.raw_body.clone();
        site_index.notes[1].raw_body = note_b.raw_body.clone();

        let data = SiteData {
            config,
            site_index,
            search_entries: entries,
            base_url,
        };

        AppState {
            output_dir: PathBuf::from("docs"),
            data: Arc::new(RwLock::new(data)),
        }
    }

    #[tokio::test]
    async fn api_search_returns_json() {
        let state = sample_state();
        let params = SearchParams {
            q: Some("rust".into()),
            limit: Some(5),
            types: None,
            tags: None,
        };

        let response = api_search(State(state), Query(params)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["schema_version"], "2024-11-llm-v1");
        let results = value["data"]["results"].as_array().expect("results array");
        assert!(!results.is_empty());
        assert_eq!(results[0]["title"], "Note A");
        assert!(results[0]["outgoing"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("note-b")));
    }

    #[tokio::test]
    async fn api_graph_neighbors_returns_nodes() {
        let state = sample_state();
        let params = GraphParams {
            depth: Some(1),
            direction: Some("both".into()),
        };

        let response =
            api_graph_neighbors(AxumPath("note-a".into()), State(state), Query(params)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["schema_version"], "2024-11-llm-v1");
        let nodes = value["data"]["nodes"].as_array().expect("nodes array");
        assert!(nodes.iter().any(|n| n["slug"] == "note-b"));
    }
}
