//! Loro-based CRDT document store for collaborative editing.
//!
//! Uses:
//! - MovableTree for document structure (sections/blocks)
//! - Richtext (Fugue-based) for per-block text
//! - Peritext-style marks for formatting

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use loro::{ExportMode, LoroDoc, LoroTree};
use serde_json::Value;
use tokio::sync::{broadcast, RwLock};

/// Map of slug -> live Loro document
#[derive(Default)]
pub struct DocStore {
    docs: RwLock<HashMap<String, Arc<LoroNoteDoc>>>,
}

impl DocStore {
    pub async fn get_or_load(
        &self,
        slug: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<Arc<LoroNoteDoc>> {
        if let Some(doc) = self.docs.read().await.get(slug).cloned() {
            return Ok(doc);
        }

        let (frontmatter, body) = load_note_from_disk(slug, site_config).await?;
        let doc = Arc::new(LoroNoteDoc::new_with_content(frontmatter, &body)?);

        // If a .loro snapshot exists, import it
        if let Some(snapshot) = load_loro_snapshot(slug, site_config).await? {
            doc.import_snapshot(&snapshot)?;
        }

        let mut guard = self.docs.write().await;
        Ok(guard
            .entry(slug.to_string())
            .or_insert_with(|| doc.clone())
            .clone())
    }

    pub async fn snapshot(&self, slug: &str, site_config: &monowiki_core::Config) -> Result<(Value, String)> {
        let doc = self.get_or_load(slug, site_config).await?;
        doc.snapshot()
    }

    pub async fn get_markdown(&self, slug: &str, site_config: &monowiki_core::Config) -> Result<String> {
        let (frontmatter, body) = self.snapshot(slug, site_config).await?;
        let yaml = serde_yaml::to_string(&frontmatter)?;
        Ok(format!("---\n{}---\n{}", yaml, body))
    }

    pub async fn flush_dirty_to_disk(&self, site_config: &monowiki_core::Config) -> Result<()> {
        let docs: Vec<_> = self
            .docs
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (slug, doc) in docs {
            if doc.is_dirty() {
                let (fm, body) = doc.snapshot()?;
                write_snapshot_to_disk(&slug, &fm, &body, site_config).await?;
                write_loro_snapshot(&slug, &doc, site_config).await?;
                doc.mark_clean();
            }
        }
        Ok(())
    }

    /// Overwrite body/frontmatter for a slug and broadcast a full update to connected peers.
    pub async fn overwrite_from_plain(
        &self,
        slug: &str,
        frontmatter: Value,
        body: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<()> {
        let doc = self.get_or_load(slug, site_config).await?;
        doc.replace_body_and_frontmatter(frontmatter, body).await;
        Ok(())
    }
}

/// A collaborative document using Loro.
pub struct LoroNoteDoc {
    doc: LoroDoc,
    frontmatter: RwLock<Value>,
    tx: broadcast::Sender<SyncPacket>,
    dirty: std::sync::atomic::AtomicBool,
    session_counter: std::sync::atomic::AtomicU64,
}

impl LoroNoteDoc {
    /// Create empty document
    pub fn new() -> Self {
        let doc = LoroDoc::new();
        let (tx, _) = broadcast::channel(128);
        Self {
            doc,
            frontmatter: RwLock::new(Value::Object(Default::default())),
            tx,
            dirty: std::sync::atomic::AtomicBool::new(false),
            session_counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Create document with initial content
    pub fn new_with_content(frontmatter: Value, body: &str) -> Result<Self> {
        let this = Self::new();

        // Initialize document structure with a single root text container
        {
            // For now, use a simple text container for the body
            // TODO: Implement proper MovableTree structure with blocks
            let text = this.doc.get_text("body");
            text.insert(0, body)?;
        }

        *this.frontmatter.blocking_write() = frontmatter;
        Ok(this)
    }

    pub fn next_session_id(&self) -> u64 {
        self.session_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// Get document tree structure (future: use MovableTree)
    pub fn get_tree(&self) -> Result<LoroTree> {
        // Future: return self.doc.get_tree("structure")
        Err(anyhow!("Tree structure not yet implemented"))
    }

    /// Get text for the main body (future: per-block text)
    pub fn get_body_text(&self) -> String {
        let text = self.doc.get_text("body");
        text.to_string()
    }

    /// Get text for a specific block (future implementation)
    pub fn get_block_text(&self, _block_id: &str) -> Result<String> {
        // Future: get text container for specific block
        Err(anyhow!("Block-level text not yet implemented"))
    }

    /// Insert text in the main body
    pub fn insert_text(&self, offset: usize, content: &str) -> Result<()> {
        let text = self.doc.get_text("body");
        text.insert(offset, content)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Delete text in the main body
    pub fn delete_text(&self, start: usize, len: usize) -> Result<()> {
        let text = self.doc.get_text("body");
        text.delete(start, len)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Add formatting mark (future: Peritext-style)
    pub fn add_mark(&self, _mark_type: &str, _start: usize, _end: usize) -> Result<()> {
        // Future: use Peritext-style marks
        // text.mark(start..end, mark_type, true.into())?;
        Err(anyhow!("Marks not yet implemented"))
    }

    /// Remove formatting mark (future: Peritext-style)
    pub fn remove_mark(&self, _mark_type: &str, _start: usize, _end: usize) -> Result<()> {
        // Future: use Peritext-style marks
        // text.unmark(start..end, mark_type)?;
        Err(anyhow!("Marks not yet implemented"))
    }

    /// Export full state for sync
    pub fn export_snapshot(&self) -> Vec<u8> {
        self.doc.export(ExportMode::Snapshot)
    }

    /// Import snapshot from another peer
    pub fn import_snapshot(&self, data: &[u8]) -> Result<()> {
        self.doc.import(data)?;
        self.mark_dirty();
        Ok(())
    }

    /// Export updates for sync
    pub fn export_updates(&self) -> Vec<u8> {
        // Loro's export format for incremental updates
        self.export_snapshot()
    }

    /// Apply updates from another peer
    pub fn apply_updates(&self, data: &[u8]) -> Result<()> {
        self.doc.import(data)?;
        self.mark_dirty();
        self.broadcast_update()?;
        Ok(())
    }

    /// Get current version (for sync protocol)
    pub fn version(&self) -> Vec<u8> {
        self.doc.oplog_vv().encode()
    }

    /// Replace body/frontmatter and emit a full update for connected peers.
    pub async fn replace_body_and_frontmatter(&self, frontmatter: Value, body: &str) {
        {
            let mut guard = self.frontmatter.write().await;
            *guard = frontmatter;
        }
        // Overwrite text in the shared doc
        self.replace_text(body);
        self.mark_dirty();

        // Broadcast full update to connected peers
        let update = self.export_snapshot();
        let _ = self.broadcast(update, 0);
    }

    fn replace_text(&self, body: &str) {
        let text = self.doc.get_text("body");
        let len = text.len_utf16();
        if len > 0 {
            let _ = text.delete(0, len);
        }
        let _ = text.insert(0, body);
    }

    /// Snapshot to frontmatter + body string
    pub fn snapshot(&self) -> Result<(Value, String)> {
        let fm = self.frontmatter.blocking_read().clone();
        let body = self.get_body_text();
        Ok((fm, body))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SyncPacket> {
        self.tx.subscribe()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn mark_dirty(&self) {
        self.dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn mark_clean(&self) {
        self.dirty
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn broadcast(&self, payload: Vec<u8>, sender_id: u64) {
        let _ = self.tx.send(SyncPacket {
            sender_id,
            payload,
        });
    }

    fn broadcast_update(&self) -> Result<()> {
        let update = self.export_snapshot();
        let _ = self.tx.send(SyncPacket {
            payload: update,
            sender_id: 0,
        });
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SyncPacket {
    pub sender_id: u64,
    pub payload: Vec<u8>,
}

// Helper functions for disk I/O

/// Convert a slug to a vault-relative path, rejecting traversal.
pub fn slug_to_rel(slug: &str) -> Result<PathBuf> {
    use std::path::Component;

    let candidate = PathBuf::from(slug.trim_matches('/'));
    let mut clean = PathBuf::new();
    for comp in candidate.components() {
        match comp {
            Component::CurDir => continue,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("invalid path component in slug"))
            }
            Component::Normal(s) => clean.push(s),
        }
    }

    if clean.as_os_str().is_empty() {
        return Err(anyhow!("empty slug"));
    }

    // Default extension to .md
    if clean.extension().is_none() {
        clean.set_extension("md");
    }

    Ok(clean)
}

async fn load_note_from_disk(slug: &str, config: &monowiki_core::Config) -> Result<(Value, String)> {
    let path = config.vault_dir().join(slug_to_rel(slug)?);
    let content = tokio::fs::read_to_string(&path).await?;
    let (fm, body) = monowiki_core::frontmatter::parse_frontmatter(&content)?;
    Ok((serde_json::to_value(fm)?, body))
}

async fn load_loro_snapshot(slug: &str, config: &monowiki_core::Config) -> Result<Option<Vec<u8>>> {
    let mut path = PathBuf::from(".collab").join(slug_to_rel(slug)?);
    path.set_extension("loro");
    let full = config.vault_dir().join(&path);
    match tokio::fs::read(&full).await {
        Ok(data) => Ok(Some(data)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

async fn write_snapshot_to_disk(
    slug: &str,
    fm: &Value,
    body: &str,
    config: &monowiki_core::Config,
) -> Result<()> {
    let path = config.vault_dir().join(slug_to_rel(slug)?);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let yaml = serde_yaml::to_string(fm)?;
    let content = format!("---\n{}---\n{}", yaml, body);
    tokio::fs::write(&path, content).await?;
    Ok(())
}

async fn write_loro_snapshot(slug: &str, doc: &LoroNoteDoc, config: &monowiki_core::Config) -> Result<()> {
    let mut path = PathBuf::from(".collab").join(slug_to_rel(slug)?);
    path.set_extension("loro");
    let full = config.vault_dir().join(&path);
    if let Some(parent) = full.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&full, doc.export_snapshot()).await?;
    Ok(())
}
