use std::path::Path as StdPath;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    body::Body,
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Path, State,
    },
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use axum_extra::extract::Multipart;
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Embedded editor UI (built from /editor with bun)
static EDITOR_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../editor/dist");

use crate::{
    auth::{AuthError, AuthState, Capability, MaybeClaims},
    build::{BuildRunner, BuildSummary},
    config::CollabConfig,
    crdt::{slug_to_rel, DocStore, SyncPacket},
    git::{GitWorkspace, GitWorkspaceSummary},
    ratelimit::RateLimiter,
    render::SharedRenderCache,
};
// Loro sync protocol: simple binary message passing

use monowiki_core::Frontmatter;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<CollabConfig>,
    pub workspace: Arc<GitWorkspace>,
    pub builder: Arc<BuildRunner>,
    pub site_config: Arc<monowiki_core::Config>,
    pub crdt: Arc<DocStore>,
    pub rate_limiter: Arc<RateLimiter>,
    pub render_cache: SharedRenderCache,
}

pub async fn serve(config: CollabConfig, workspace: GitWorkspace, builder: BuildRunner) -> Result<()> {
    let config = Arc::new(config);
    let workspace = Arc::new(workspace);
    let builder = Arc::new(builder);

    // Ensure the worktree exists before serving.
    workspace.init_or_refresh().await?;
    let site_config = monowiki_core::Config::from_file(config.config_path())?;
    let crdt = Arc::new(DocStore::default());
    let render_cache = SharedRenderCache::new();

    // Initialize render cache with site config
    render_cache.initialize(site_config.clone()).await;

    if config.build_on_start {
        builder.ensure_ready().await?;
        builder.run_build().await?;

        // Load site index into render cache for incremental rendering
        let site_builder = monowiki_core::SiteBuilder::new(site_config.clone());
        if let Ok(index) = site_builder.build() {
            render_cache.load_site_index(index).await;
        }
    }

    // Build auth state
    let auth_state = AuthState {
        keystore: Arc::new(config.auth.build_keystore()),
        require_auth: config.auth.require_auth,
    };

    // Build rate limiter
    let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit.clone()));

    let state = AppState {
        config: config.clone(),
        workspace: workspace.clone(),
        builder: builder.clone(),
        site_config: Arc::new(site_config),
        crdt: crdt.clone(),
        rate_limiter,
        render_cache,
    };

    let app = Router::new()
        // Editor UI (served from embedded dist)
        .route("/", get(serve_editor_index))
        .route("/assets/{*path}", get(serve_editor_asset))
        // Preview (served from build output)
        .route("/preview", get(serve_preview_index))
        .route("/preview/", get(serve_preview_index))
        .route("/preview/{*path}", get(serve_preview_file))
        // Serve preview assets at root level too (for iframe compatibility)
        .route("/css/{*path}", get(serve_preview_css))
        .route("/js/{*path}", get(serve_preview_js))
        // Public endpoints (no auth required)
        .route("/healthz", get(healthz))
        // Authenticated endpoints
        .route("/api/status", get(status))
        .route("/api/files", get(list_files))
        .route("/api/note/{*slug}", get(get_note).put(write_note))
        .route("/api/checkpoint", post(checkpoint))
        .route("/api/build", post(build_now))
        .route("/api/flush", post(flush_now))
        .route("/api/upload", post(upload_asset))
        .route("/api/render/{*slug}", post(render_single))
        .route("/ws/note/{*slug}", get(ws_note))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50MB max for uploads
        .layer(Extension(auth_state))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    let url = format!("http://{}", config.listen_addr);
    info!(addr = %config.listen_addr, "monowiki-collab listening");
    println!("\nEditor:  {}", url);
    println!("Preview: {}/preview", url);
    println!("API:     {}/api/status", url);
    println!("Press Ctrl+C to stop\n");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

// ─────────────────────────────────────────────────────────────────────────────
// Editor UI (embedded)
// ─────────────────────────────────────────────────────────────────────────────

async fn serve_editor_index() -> Response {
    match EDITOR_DIST.get_file("index.html") {
        Some(file) => Html(String::from_utf8_lossy(file.contents()).to_string()).into_response(),
        None => (StatusCode::NOT_FOUND, "Editor not built - run `bun run build` in /editor").into_response(),
    }
}

async fn serve_editor_asset(Path(path): Path<String>) -> Response {
    let file_path = format!("assets/{}", path);
    match EDITOR_DIST.get_file(&file_path) {
        Some(file) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type_for_path(&path))
            .body(Body::from(file.contents().to_vec()))
            .unwrap(),
        None => (StatusCode::NOT_FOUND, "Asset not found").into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Preview (served from build output)
// ─────────────────────────────────────────────────────────────────────────────

async fn serve_preview_index(State(state): State<AppState>) -> Response {
    let index_path = state.site_config.output_dir().join("index.html");
    match tokio::fs::read_to_string(&index_path).await {
        Ok(content) => Html(content).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Preview not built - click Build first").into_response(),
    }
}

async fn serve_preview_file(Path(path): Path<String>, State(state): State<AppState>) -> Response {
    // Handle empty path or trailing slash as index
    if path.is_empty() || path == "/" {
        let index_path = state.site_config.output_dir().join("index.html");
        return match tokio::fs::read_to_string(&index_path).await {
            Ok(content) => Html(content).into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "Preview not built").into_response(),
        };
    }

    let file_path = state.site_config.output_dir().join(&path);
    match tokio::fs::read(&file_path).await {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type_for_path(&path))
            .body(Body::from(content))
            .unwrap(),
        Err(_) => {
            // Try with .html extension for clean URLs
            let html_path = state.site_config.output_dir().join(format!("{}.html", path));
            match tokio::fs::read(&html_path).await {
                Ok(content) => Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(Body::from(content))
                    .unwrap(),
                Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
            }
        }
    }
}

/// Serve CSS files from preview output at /css/* (for iframe compatibility)
async fn serve_preview_css(Path(path): Path<String>, State(state): State<AppState>) -> Response {
    let file_path = state.site_config.output_dir().join("css").join(&path);
    match tokio::fs::read(&file_path).await {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type_for_path(&path))
            .body(Body::from(content))
            .unwrap(),
        Err(_) => (StatusCode::NOT_FOUND, "CSS not found").into_response(),
    }
}

/// Serve JS files from preview output at /js/* (for iframe compatibility)
async fn serve_preview_js(Path(path): Path<String>, State(state): State<AppState>) -> Response {
    let file_path = state.site_config.output_dir().join("js").join(&path);
    match tokio::fs::read(&file_path).await {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/javascript; charset=utf-8")
            .body(Body::from(content))
            .unwrap(),
        Err(_) => (StatusCode::NOT_FOUND, "JS not found").into_response(),
    }
}

fn content_type_for_path(path: &str) -> &'static str {
    match StdPath::new(path)
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
        "map" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[derive(Serialize)]
struct StatusPayload {
    listening: String,
    repo: GitWorkspaceSummary,
    build: BuildSummary,
    config_path: String,
}

async fn status(
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
) -> Result<impl IntoResponse, AuthError> {
    // Check auth: need Read capability (no slug)
    if let Some(ref c) = claims {
        c.authorize(Capability::Read, None)?;
    }

    let body = StatusPayload {
        listening: state.config.listen_addr.clone(),
        repo: state.workspace.repo_summary(),
        build: state.builder.summary(),
        config_path: state.config.config_path().display().to_string(),
    };

    Ok(Json(body))
}

/// List all markdown files in the vault
async fn list_files(
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
) -> Result<impl IntoResponse, AuthError> {
    if let Some(ref c) = claims {
        c.authorize(Capability::Read, None)?;
    }

    let vault_dir = state.site_config.vault_dir();
    let mut files = Vec::new();

    if let Ok(entries) = collect_md_files(&vault_dir, &vault_dir) {
        files = entries;
    }

    Ok(Json(serde_json::json!({ "files": files })))
}

/// Recursively collect .md files from a directory
fn collect_md_files(base: &std::path::Path, dir: &std::path::Path) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/directories
        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            // Recurse into subdirectory
            if let Ok(children) = collect_md_files(base, &path) {
                if !children.is_empty() {
                    let rel_path = path.strip_prefix(base).unwrap_or(&path);
                    entries.push(FileEntry {
                        name,
                        path: rel_path.to_string_lossy().to_string(),
                        is_dir: true,
                        children: Some(children),
                    });
                }
            }
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            let rel_path = path.strip_prefix(base).unwrap_or(&path);
            // Convert path to slug (remove .md extension)
            let slug = rel_path
                .with_extension("")
                .to_string_lossy()
                .to_string();
            entries.push(FileEntry {
                name,
                path: slug,
                is_dir: false,
                children: None,
            });
        }
    }

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(entries)
}

#[derive(Serialize)]
struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<FileEntry>>,
}

#[derive(Serialize)]
struct NoteResponse {
    slug: String,
    path: String,
    frontmatter: serde_json::Value,
    body: String,
}

async fn get_note(
    Path(slug): Path<String>,
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
) -> Result<impl IntoResponse, AuthError> {
    // Check auth if configured
    if let Some(ref c) = claims {
        c.authorize(Capability::Read, Some(&slug))?;
    }

    match read_note(&state, &slug).await {
        Ok(note) => Ok(Json(note).into_response()),
        Err(err) => {
            warn!(%slug, ?err, "failed to read note");
            Ok((StatusCode::NOT_FOUND, format!("note not found: {slug}")).into_response())
        }
    }
}

#[derive(Deserialize)]
struct WriteNoteRequest {
    frontmatter: Option<serde_json::Value>,
    body: String,
    /// Optional explicit path (relative to vault). Defaults to slug.md
    path: Option<String>,
    /// If true, run checkpoint (add/commit/push) after write.
    checkpoint: Option<bool>,
}

#[derive(Serialize)]
struct WriteNoteResponse {
    path: String,
    checkpointed: bool,
}

async fn write_note(
    Path(slug): Path<String>,
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
    Json(payload): Json<WriteNoteRequest>,
) -> Result<impl IntoResponse, AuthError> {
    // Check auth: need Write capability
    if let Some(ref c) = claims {
        c.authorize(Capability::Write, Some(&slug))?;
        // If checkpointing, also need Checkpoint capability
        if payload.checkpoint.unwrap_or(false) {
            c.authorize(Capability::Checkpoint, None)?;
        }
    }

    // Check rate limit using identity from claims
    let identity = claims.as_ref().map(|c| c.sub.as_str()).unwrap_or("anonymous");
    check_rate_limit(&state, identity).await?;

    match write_note_to_disk(&state, &slug, payload).await {
        Ok(resp) => Ok(Json(resp).into_response()),
        Err(err) => {
            warn!(%slug, ?err, "failed to write note");
            Ok((
                StatusCode::BAD_REQUEST,
                format!("failed to write note: {err}"),
            )
                .into_response())
        }
    }
}

#[derive(Serialize)]
struct CheckpointResponse {
    committed: bool,
    message: String,
}

async fn checkpoint(
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
) -> Result<impl IntoResponse, AuthError> {
    // Check auth: need Checkpoint capability
    if let Some(ref c) = claims {
        c.authorize(Capability::Checkpoint, None)?;
    }

    // Check rate limit
    let identity = claims.as_ref().map(|c| c.sub.as_str()).unwrap_or("anonymous");
    check_rate_limit(&state, identity).await?;

    if let Err(err) = flush_crdt_to_disk(&state).await {
        warn!(?err, "failed to flush CRDT docs before checkpoint");
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("flush failed: {err}"),
        )
            .into_response());
    }

    match commit_and_push(&state, "collab checkpoint").await {
        Ok(committed) => Ok(Json(CheckpointResponse {
            committed,
            message: if committed {
                "changes pushed".to_string()
            } else {
                "no changes to commit".to_string()
            },
        })
        .into_response()),
        Err(err) => {
            warn!(?err, "checkpoint failed");
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("checkpoint failed: {err}"),
            )
                .into_response())
        }
    }
}

async fn build_now(
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
) -> Result<impl IntoResponse, AuthError> {
    // Check auth: need Build capability
    if let Some(ref c) = claims {
        c.authorize(Capability::Build, None)?;
    }

    // Check rate limit
    let identity = claims.as_ref().map(|c| c.sub.as_str()).unwrap_or("anonymous");
    check_rate_limit(&state, identity).await?;

    if let Err(err) = flush_crdt_to_disk(&state).await {
        warn!(?err, "failed to flush CRDT docs before build");
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("flush failed: {err}"),
        ));
    }

    if let Err(err) = state.builder.run_build().await {
        let msg = format!("Build failed: {err:?}");
        return Ok((StatusCode::INTERNAL_SERVER_ERROR, msg));
    }

    Ok((StatusCode::ACCEPTED, "Build completed".to_string()))
}

/// Flush dirty CRDT docs to disk without committing.
async fn flush_now(
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
) -> Result<impl IntoResponse, AuthError> {
    // Require Write capability (same as editing)
    if let Some(ref c) = claims {
        c.authorize(Capability::Write, None)?;
    }

    let identity = claims.as_ref().map(|c| c.sub.as_str()).unwrap_or("anonymous");
    check_rate_limit(&state, identity).await?;

    if let Err(err) = flush_crdt_to_disk(&state).await {
        warn!(?err, "failed to flush");
        return Ok((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("flush failed: {err}")})),
        ));
    }

    Ok((StatusCode::OK, Json(serde_json::json!({"message": "Flushed dirty docs"}))))
}

/// Render a single note incrementally (without full rebuild).
/// Gets markdown from CRDT and renders to HTML using cached site context.
async fn render_single(
    Path(slug): Path<String>,
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
) -> Result<impl IntoResponse, AuthError> {
    // Check auth: need Read capability
    if let Some(ref c) = claims {
        c.authorize(Capability::Read, Some(&slug))?;
    }

    // Get markdown content from CRDT
    let markdown = match state.crdt.get_markdown(&slug, &state.site_config).await {
        Ok(md) => md,
        Err(err) => {
            warn!(%slug, ?err, "failed to get markdown for render");
            return Ok((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("note not found: {slug}")})),
            ).into_response());
        }
    };

    // Render using cached context
    match state.render_cache.render_single(&slug, &markdown).await {
        Ok(_html) => {
            info!(%slug, "incremental render complete");
            Ok(Json(serde_json::json!({
                "slug": slug,
                "success": true
            })).into_response())
        }
        Err(err) => {
            warn!(%slug, ?err, "incremental render failed");
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("render failed: {err}")})),
            ).into_response())
        }
    }
}

async fn ws_note(
    Path(slug): Path<String>,
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, AuthError> {
    // Check auth: need Read capability for the slug
    if let Some(ref c) = claims {
        c.authorize(Capability::Read, Some(&slug))?;
    }

    Ok(ws.on_upgrade(move |socket| async move {
        if let Err(err) = handle_ws(socket, slug, state).await {
            warn!(?err, "websocket session ended with error");
        }
    }))
}

async fn read_note(state: &AppState, slug: &str) -> Result<NoteResponse> {
    let rel_path = slug_to_rel(slug)?;
    let (frontmatter_json, body) = state.crdt.snapshot(slug, &state.site_config).await?;

    Ok(NoteResponse {
        slug: slug.to_string(),
        path: rel_path.to_string_lossy().to_string(),
        frontmatter: frontmatter_json,
        body: body.to_string(),
    })
}

async fn write_note_to_disk(
    state: &AppState,
    slug: &str,
    payload: WriteNoteRequest,
) -> Result<WriteNoteResponse> {
    let rel_path = if let Some(path) = payload.path {
        slug_to_rel(&path)?
    } else {
        slug_to_rel(slug)?
    };
    let abs_path = state.site_config.vault_dir().join(&rel_path);
    if let Some(parent) = abs_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let frontmatter: Frontmatter = if let Some(value) = payload.frontmatter {
        serde_json::from_value(value)?
    } else {
        Frontmatter {
            title: slug.to_string(),
            ..Frontmatter::default()
        }
    };

    let yaml = serde_yaml::to_string(&frontmatter)?;
    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&yaml);
    content.push_str("---\n");
    content.push_str(&payload.body);

    tokio::fs::write(&abs_path, content).await?;

    // Update CRDT doc and broadcast a full update to connected peers.
    let fm_json = serde_json::to_value(&frontmatter)?;
    state
        .crdt
        .overwrite_from_plain(slug, fm_json, &payload.body, &state.site_config)
        .await?;

    let checkpointed = if payload.checkpoint.unwrap_or(false) {
        commit_and_push(state, "collab save").await?
    } else {
        false
    };

    Ok(WriteNoteResponse {
        path: rel_path.to_string_lossy().to_string(),
        checkpointed,
    })
}

async fn commit_and_push(state: &AppState, message: &str) -> Result<bool> {
    flush_crdt_to_disk(state).await?;
    state.workspace.init_or_refresh().await?;
    state.workspace.add_vault().await?;
    let committed = state.workspace.commit(message, false).await?;
    if committed {
        state.workspace.push().await?;
    }
    Ok(committed)
}

async fn flush_crdt_to_disk(state: &AppState) -> Result<()> {
    state.crdt.flush_dirty_to_disk(&state.site_config).await?;
    Ok(())
}

/// Check rate limit for a given identity (from JWT sub claim).
async fn check_rate_limit(state: &AppState, identity: &str) -> Result<(), AuthError> {
    if let Err(retry_after) = state.rate_limiter.check(identity).await {
        return Err(AuthError::RateLimited {
            retry_after_secs: retry_after.as_secs().max(1),
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Asset upload
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum upload size per file (50MB)
const MAX_UPLOAD_SIZE: usize = 50 * 1024 * 1024;

#[derive(Serialize)]
struct UploadResponse {
    path: String,
    url: String,
    original_name: String,
}

/// Upload an asset to vault/assets/...
/// Accepts multipart form data with a file field.
/// Files are renamed to UUID to avoid collisions.
async fn upload_asset(
    State(state): State<AppState>,
    MaybeClaims(claims): MaybeClaims,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AuthError> {
    // Check auth: need Write capability (no specific slug for uploads)
    if let Some(ref c) = claims {
        c.authorize(Capability::Write, None)?;
    }

    // Check rate limit
    let identity = claims.as_ref().map(|c| c.sub.as_str()).unwrap_or("anonymous");
    check_rate_limit(&state, identity).await?;

    // Process multipart form
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }

        let original_filename = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "upload".to_string());

        // Extract and validate extension from original filename
        let extension = extract_safe_extension(&original_filename);

        // Read file data with size limit
        let data = match field.bytes().await {
            Ok(d) => {
                if d.len() > MAX_UPLOAD_SIZE {
                    return Ok((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Json(serde_json::json!({
                            "error": "file too large",
                            "max_size": MAX_UPLOAD_SIZE
                        })),
                    )
                        .into_response());
                }
                d
            }
            Err(e) => {
                warn!(?e, "failed to read upload data");
                return Ok((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": "failed to read file data"})),
                )
                    .into_response());
            }
        };

        // Generate UUID-based filename to avoid collisions
        let uuid = uuid::Uuid::new_v4();
        let safe_filename = if let Some(ext) = extension {
            format!("{}.{}", uuid, ext)
        } else {
            uuid.to_string()
        };

        // Write to vault/assets/
        let assets_dir = state.site_config.vault_dir().join("assets");
        if let Err(e) = tokio::fs::create_dir_all(&assets_dir).await {
            warn!(?e, "failed to create assets directory");
            return Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "failed to create assets directory"})),
            )
                .into_response());
        }

        let file_path = assets_dir.join(&safe_filename);
        if let Err(e) = tokio::fs::write(&file_path, &data).await {
            warn!(?e, "failed to write asset file");
            return Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "failed to write file"})),
            )
                .into_response());
        }

        let rel_path = format!("assets/{}", safe_filename);
        let url = format!("/vault/{}", rel_path);

        info!(
            path = %rel_path,
            size = data.len(),
            original = %original_filename,
            "asset uploaded"
        );

        return Ok(Json(UploadResponse {
            path: rel_path,
            url,
            original_name: original_filename,
        })
        .into_response());
    }

    Ok((
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": "no file field in request"})),
    )
        .into_response())
}

/// Extract a safe file extension from a filename.
/// Returns None if extension is invalid or potentially dangerous.
fn extract_safe_extension(name: &str) -> Option<String> {
    // Get just the filename (no path)
    let name = std::path::Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())?;

    // Check for path traversal attempts
    if name.contains("..") || name.starts_with('.') && name.len() > 1 && !name.contains('.') {
        return None;
    }

    // Get extension
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|s| s.to_str())?;

    // Validate extension: alphanumeric only, reasonable length
    if ext.len() > 10 || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }

    Some(ext.to_lowercase())
}

async fn handle_ws(mut socket: WebSocket, slug: String, state: AppState) -> Result<()> {
    let doc = state.crdt.get_or_load(&slug, &state.site_config).await?;
    let session_id = doc.next_session_id();
    let mut broadcast_rx = doc.subscribe();

    // Send initial snapshot for sync
    let init_snapshot = doc.export_snapshot();
    if !init_snapshot.is_empty() {
        socket
            .send(WsMessage::Binary(init_snapshot.into()))
            .await
            .ok();
    }

    loop {
        tokio::select! {
            // Messages from other peers (Loro updates)
            recv = broadcast_rx.recv() => {
                match recv {
                    Ok(SyncPacket { sender_id, payload }) => {
                        if sender_id != session_id {
                            if socket.send(WsMessage::Binary(payload.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            // Messages from this client
            inbound = socket.recv() => {
                match inbound {
                    Some(Ok(WsMessage::Binary(bytes))) => {
                        if bytes.is_empty() { continue; }

                        // Loro sync: client sends updates, we apply and broadcast
                        match doc.apply_updates(&bytes) {
                            Ok(()) => {
                                // Update applied successfully
                                // The broadcast happens inside apply_updates
                            }
                            Err(err) => {
                                warn!(%slug, ?err, "failed to apply Loro update");
                            }
                        }
                    }
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    Some(Ok(WsMessage::Ping(p))) => { let _ = socket.send(WsMessage::Pong(p)).await; }
                    Some(Ok(_)) => {}
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
