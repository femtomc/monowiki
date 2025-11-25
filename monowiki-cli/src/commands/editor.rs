//! Editor server command - serves the collaborative editor UI.

use anyhow::{Context, Result};
use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use include_dir::{include_dir, Dir};
use std::path::Path;

/// Embedded editor dist files (built from /editor with bun)
static EDITOR_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../editor/dist");

/// Start the editor server.
pub async fn editor_server(port: Option<u16>, open_browser: bool) -> Result<()> {
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/{*path}", get(serve_file));

    let bind_port = port.unwrap_or(5173);
    let addr = format!("127.0.0.1:{}", bind_port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind to {}", addr))?;

    let actual_port = listener.local_addr()?.port();
    let url = format!("http://localhost:{}", actual_port);

    tracing::info!("Starting editor server on {}", url);
    println!("\nEditor at {}", url);
    println!("Make sure collab daemon is running (monowiki collab ...)");
    println!("Press Ctrl+C to stop\n");

    if open_browser {
        if let Err(e) = open::that(&url) {
            tracing::warn!("Failed to open browser: {}", e);
        }
    }

    axum::serve(listener, app).await.context("Server error")?;

    Ok(())
}

async fn serve_index() -> Response {
    match EDITOR_DIST.get_file("index.html") {
        Some(file) => Html(String::from_utf8_lossy(file.contents()).to_string()).into_response(),
        None => (StatusCode::NOT_FOUND, "Editor not built").into_response(),
    }
}

async fn serve_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try exact path first
    if let Some(file) = EDITOR_DIST.get_file(path) {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type_for_path(path))
            .body(Body::from(file.contents().to_vec()))
            .unwrap();
    }

    // Try with index.html for SPA routing
    if let Some(file) = EDITOR_DIST.get_file("index.html") {
        return Html(String::from_utf8_lossy(file.contents()).to_string()).into_response();
    }

    (StatusCode::NOT_FOUND, "File not found").into_response()
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
        "map" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}
