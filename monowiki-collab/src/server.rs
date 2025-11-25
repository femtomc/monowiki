use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_extra::extract::Multipart;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::{
    auth::{AuthError, AuthState, Capability, MaybeClaims},
    build::{BuildRunner, BuildSummary},
    config::CollabConfig,
    git::{GitWorkspace, GitWorkspaceSummary},
    crdt::{BroadcastPacket, DocStore, slug_to_rel},
    ratelimit::RateLimiter,
};
use yrs::sync::AwarenessUpdate;
use yrs::updates::decoder::{DecoderV1, Decode};
use yrs::encoding::read::Cursor;

use monowiki_core::Frontmatter;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<CollabConfig>,
    pub workspace: Arc<GitWorkspace>,
    pub builder: Arc<BuildRunner>,
    pub site_config: Arc<monowiki_core::Config>,
    pub crdt: Arc<DocStore>,
    pub rate_limiter: Arc<RateLimiter>,
}

pub async fn serve(config: CollabConfig, workspace: GitWorkspace, builder: BuildRunner) -> Result<()> {
    let config = Arc::new(config);
    let workspace = Arc::new(workspace);
    let builder = Arc::new(builder);

    // Ensure the worktree exists before serving.
    workspace.init_or_refresh().await?;
    let site_config = monowiki_core::Config::from_file(config.config_path())?;
    let crdt = Arc::new(DocStore::default());

    if config.build_on_start {
        builder.ensure_ready().await?;
        builder.run_build().await?;
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
    };

    let app = Router::new()
        // Public endpoints (no auth required)
        .route("/healthz", get(healthz))
        // Authenticated endpoints
        .route("/api/status", get(status))
        .route("/api/note/{*slug}", get(get_note).put(write_note))
        .route("/api/checkpoint", post(checkpoint))
        .route("/api/build", post(build_now))
        .route("/api/flush", post(flush_now))
        .route("/api/upload", post(upload_asset))
        .route("/ws/note/{*slug}", get(ws_note))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50MB max for uploads
        .layer(Extension(auth_state))
        .with_state(state);

    info!(addr = %config.listen_addr, "monowiki-collab listening");
    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
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
            format!("flush failed: {err}"),
        ));
    }

    Ok((StatusCode::OK, "Flushed dirty docs".to_string()))
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

    // Send initial sync payload (state vector + awareness).
    if let Ok(init) = doc.start_sync_payload() {
        if !init.is_empty() {
            let mut framed = Vec::with_capacity(1 + init.len());
            framed.push(0);
            framed.extend_from_slice(&init);
            socket.send(WsMessage::Binary(framed.into())).await.ok();
        }
    }
    if let Ok(Some(aw)) = doc.awareness_update() {
        let mut framed = Vec::with_capacity(1 + aw.len());
        framed.push(1);
        framed.extend_from_slice(&aw);
        socket.send(WsMessage::Binary(framed.into())).await.ok();
    }

    loop {
        tokio::select! {
            // Messages from other peers
            recv = broadcast_rx.recv() => {
                match recv {
                    Ok(BroadcastPacket { sender_id, frame_type, payload }) => {
                        if sender_id != session_id {
                            let mut framed = Vec::with_capacity(1 + payload.len());
                            framed.push(frame_type);
                            framed.extend_from_slice(&payload);
                            if socket.send(WsMessage::Binary(framed.into())).await.is_err() {
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
                        let frame_type = bytes[0];
                        let payload = &bytes[1..];

                        if frame_type == 0 {
                            match doc.handle_sync_message(session_id, payload) {
                                Ok(responses) => {
                                    for (ft, resp) in responses {
                                        let mut framed = Vec::with_capacity(1 + resp.len());
                                        framed.push(ft);
                                        framed.extend_from_slice(&resp);
                                        if socket.send(WsMessage::Binary(framed.into())).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                                Err(err) => {
                                    warn!(%slug, ?err, "failed to handle y-sync message");
                                }
                            }
                        } else if frame_type == 1 {
                            let mut dec = DecoderV1::new(Cursor::new(payload));
                            match AwarenessUpdate::decode(&mut dec) {
                                Ok(update) => {
                                    if let Err(err) = doc.awareness().apply_update(update) {
                                        warn!(?err, %slug, "failed to apply awareness update");
                                    } else if let Ok(Some(aw)) = doc.awareness_update() {
                                        doc.broadcast_frame(1, aw, session_id);
                                    }
                                }
                                Err(err) => warn!(?err, %slug, "failed to decode awareness update"),
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
