//! Stream vault change events for agents.

use anyhow::{Context, Result};
use chrono::Utc;
use monowiki_core::{slugify, Config};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::json;
use std::path::Path;
use tokio::sync::mpsc;

pub async fn watch_changes(config_path: &Path) -> Result<()> {
    let config = Config::from_file(config_path).context("Failed to load configuration")?;
    let vault_dir = config.vault_dir();

    println!("Watching {:?} for changes (Ctrl+C to stop)...", vault_dir);

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut _watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        notify::Config::default(),
    )
    .context("Failed to initialize watcher")?;

    _watcher
        .watch(&vault_dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to watch {:?}", vault_dir))?;

    while let Some(event) = rx.recv().await {
        match event {
            Ok(ev) => {
                let event_type = describe_event(&ev.kind);
                for path in ev.paths {
                    let rel = path
                        .strip_prefix(&vault_dir)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string_lossy().to_string());

                    let slug = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(slugify)
                        .unwrap_or_default();

                    let payload = json!({
                        "event": event_type,
                        "path": rel,
                        "slug": slug,
                        "timestamp": Utc::now().to_rfc3339(),
                    });
                    println!("{}", payload);
                }
            }
            Err(err) => eprintln!("Watcher error: {err}"),
        }
    }

    Ok(())
}

fn describe_event(kind: &EventKind) -> &'static str {
    use notify::event::ModifyKind;

    match kind {
        EventKind::Create(_) => "create",
        EventKind::Modify(ModifyKind::Name(_)) => "rename",
        EventKind::Modify(_) => "modify",
        EventKind::Remove(_) => "remove",
        _ => "unknown",
    }
}
