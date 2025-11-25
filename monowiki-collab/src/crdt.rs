use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::Value;
use tokio::sync::{broadcast, RwLock};
use tracing::warn;
use yrs::encoding::read::Cursor;
use yrs::sync::{Awareness, DefaultProtocol, Message, MessageReader, Protocol, SyncMessage};
use yrs::updates::decoder::DecoderV1;
use yrs::updates::encoder::{Encode, Encoder, EncoderV1};
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, StateVector, Text, Transact, ReadTxn, WriteTxn, Update};

/// Map of slug -> live CRDT document + awareness state.
#[derive(Default)]
pub struct DocStore {
    docs: RwLock<HashMap<String, Arc<NoteDoc>>>,
}

impl DocStore {
    pub async fn get_or_load(
        &self,
        slug: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<Arc<NoteDoc>> {
        if let Some(doc) = self.docs.read().await.get(slug).cloned() {
            return Ok(doc);
        }

        let (frontmatter, body) = load_note_from_disk(slug, site_config).await?;
        let doc = Arc::new(NoteDoc::new(frontmatter, &body));

        // If a .ydoc exists, hydrate the CRDT state from it (body overrides the markdown body)
        if let Some(update_bytes) = load_ydoc_from_disk(slug, site_config).await? {
            let update = Update::decode_v1(&update_bytes)?;
            let mut txn = doc.awareness.doc().transact_mut();
            txn.apply_update(update)?;
        }

        let mut guard = self.docs.write().await;
        Ok(guard.entry(slug.to_string()).or_insert_with(|| doc.clone()).clone())
    }

    pub async fn snapshot(
        &self,
        slug: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<(Value, String)> {
        let doc = self.get_or_load(slug, site_config).await?;
        Ok(doc.snapshot().await)
    }

    /// Get complete markdown (frontmatter + body) for a slug.
    pub async fn get_markdown(
        &self,
        slug: &str,
        site_config: &monowiki_core::Config,
    ) -> Result<String> {
        let (frontmatter, body) = self.snapshot(slug, site_config).await?;
        let yaml = serde_yaml::to_string(&frontmatter)?;
        let mut content = String::new();
        content.push_str("---\n");
        content.push_str(&yaml);
        content.push_str("---\n");
        content.push_str(&body);
        Ok(content)
    }

    /// Flush all loaded docs to disk (frontmatter + markdown body).
    pub async fn flush_dirty_to_disk(&self, site_config: &monowiki_core::Config) -> Result<()> {
        let docs: Vec<(String, Arc<NoteDoc>)> = {
            let guard = self.docs.read().await;
            guard.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };

        for (slug, doc) in docs {
            if !doc.is_dirty() {
                continue;
            }
            let (frontmatter, body) = doc.snapshot().await;
            write_snapshot_to_disk(&slug, &frontmatter, &body, site_config).await?;
            write_ydoc_to_disk(&slug, &doc, site_config).await?;
            doc.mark_clean();
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

/// Represents a single collaborative note.
pub struct NoteDoc {
    awareness: Awareness,
    frontmatter: RwLock<Value>,
    tx: broadcast::Sender<BroadcastPacket>,
    session_counter: AtomicU64,
    dirty: AtomicBool,
}

impl NoteDoc {
    pub fn new(frontmatter: Value, body: &str) -> Self {
        let doc = Doc::new();
        let awareness = Awareness::new(doc.clone());
        let (tx, _) = broadcast::channel(128);
        let session_counter = AtomicU64::new(1);
        let note = Self {
            awareness,
            frontmatter: RwLock::new(frontmatter),
            tx,
            session_counter,
            dirty: AtomicBool::new(false),
        };
        note.replace_text(body);
        note
    }

    pub fn next_session_id(&self) -> u64 {
        self.session_counter.fetch_add(1, Ordering::Relaxed)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BroadcastPacket> {
        self.tx.subscribe()
    }

    pub fn awareness(&self) -> &Awareness {
        &self.awareness
    }

    /// Produce initial sync payload for a newly connected client.
    pub fn start_sync_payload(&self) -> Result<Vec<u8>> {
        let mut encoder = EncoderV1::new();
        DefaultProtocol.start(self.awareness(), &mut encoder)?;
        Ok(encoder.to_vec())
    }

    /// Current awareness state as a complete y-protocols Awareness message.
    pub fn awareness_update(&self) -> Result<Option<Vec<u8>>> {
        let update = self.awareness.update()?;
        // Wrap as a y-protocols Awareness message
        let msg = Message::Awareness(update);
        Ok(Some(encode_message(msg)))
    }

    /// Handle an incoming y-protocols message (sync or awareness), returning responses
    /// to send back to the sender. Broadcasts relevant messages to other peers.
    pub fn handle_sync_message(&self, sender_id: u64, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        // Apply to doc and gather responses for the sender
        let protocol = DefaultProtocol;
        let responses = protocol.handle(self.awareness(), data)?;
        self.dirty.store(true, Ordering::Relaxed);

        // Encode responses as complete y-protocols messages
        let mut encoded_responses = Vec::new();
        for msg in responses {
            encoded_responses.push(encode_message(msg));
        }

        // Broadcast updates/awareness messages to other peers
        for msg in filter_broadcast_messages(data) {
            let bytes = encode_message(msg);
            let _ = self.broadcast(bytes, sender_id);
        }

        Ok(encoded_responses)
    }

    /// Replace body/frontmatter and emit a full update for connected peers.
    pub async fn replace_body_and_frontmatter(&self, frontmatter: Value, body: &str) {
        {
            let mut guard = self.frontmatter.write().await;
            *guard = frontmatter;
        }
        // Overwrite text in the shared doc
        self.replace_text(body);
        self.dirty.store(true, Ordering::Relaxed);

        // Broadcast full update (from empty state vector) so connected peers refresh.
        let update = {
            let txn = self.awareness.doc().transact();
            txn.encode_state_as_update_v1(&StateVector::default())
        };
        let msg = Message::Sync(SyncMessage::Update(update));
        let bytes = encode_message(msg);
        let _ = self.broadcast(bytes, 0);
    }

    /// Snapshot current frontmatter/body from the CRDT doc.
    pub async fn snapshot(&self) -> (Value, String) {
        let fm = self.frontmatter.read().await.clone();
        let mut txn = self.awareness.doc().transact_mut();
        let text = txn.get_or_insert_text(TEXT_FIELD);
        let body = text.get_string(&txn);
        (fm, body)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    pub fn mark_clean(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    pub fn broadcast(&self, payload: Vec<u8>, sender_id: u64) {
        let _ = self.tx.send(BroadcastPacket {
            sender_id,
            payload,
        });
    }

    fn replace_text(&self, body: &str) {
        let mut txn = self.awareness.doc().transact_mut();
        let text = txn.get_or_insert_text(TEXT_FIELD);
        let len = text.len(&txn);
        if len > 0 {
            text.remove_range(&mut txn, 0, len);
        }
        text.insert(&mut txn, 0, body);
    }
}

/// Message sent over broadcast channel (y-protocols encoded payload + sender id).
#[derive(Clone, Debug)]
pub struct BroadcastPacket {
    pub sender_id: u64,
    /// Complete y-protocols encoded message
    pub payload: Vec<u8>,
}

fn encode_message(msg: Message) -> Vec<u8> {
    let mut encoder = EncoderV1::new();
    msg.encode(&mut encoder);
    encoder.to_vec()
}

/// Filter incoming y-protocols data down to messages that should be broadcast to other peers.
fn filter_broadcast_messages(data: &[u8]) -> Vec<Message> {
    let mut decoder = DecoderV1::new(Cursor::new(data));
    let mut reader = MessageReader::new(&mut decoder);
    let mut msgs = Vec::new();
    while let Some(res) = reader.next() {
        match res {
            Ok(msg @ Message::Sync(SyncMessage::Update(_))) => msgs.push(msg),
            Ok(msg @ Message::Awareness(_) | msg @ Message::AwarenessQuery) => msgs.push(msg),
            Ok(_) => {}
            Err(err) => {
                warn!(?err, "failed to decode y-protocols message");
                break;
            }
        }
    }
    msgs
}

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

async fn load_note_from_disk(
    slug: &str,
    site_config: &monowiki_core::Config,
) -> Result<(Value, String)> {
    let rel_path = slug_to_rel(slug)?;
    let abs_path = site_config.vault_dir().join(&rel_path);
    let raw = tokio::fs::read_to_string(&abs_path).await?;
    let (frontmatter, body) = monowiki_core::frontmatter::parse_frontmatter(&raw)?;
    let frontmatter_json = serde_json::to_value(frontmatter)?;
    Ok((frontmatter_json, body))
}

async fn load_ydoc_from_disk(
    slug: &str,
    site_config: &monowiki_core::Config,
) -> Result<Option<Vec<u8>>> {
    let rel_path = slug_to_rel(slug)?;
    let mut ydoc_rel = PathBuf::from(".collab").join(rel_path);
    ydoc_rel.set_extension("ydoc");
    let abs_path = site_config.vault_dir().join(&ydoc_rel);
    match tokio::fs::read(&abs_path).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(e.into())
            }
        }
    }
}

async fn write_snapshot_to_disk(
    slug: &str,
    frontmatter: &Value,
    body: &str,
    site_config: &monowiki_core::Config,
) -> Result<()> {
    let rel_path = slug_to_rel(slug)?;
    let abs_path = site_config.vault_dir().join(&rel_path);
    if let Some(parent) = abs_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let yaml = serde_yaml::to_string(frontmatter)?;
    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&yaml);
    content.push_str("---\n");
    content.push_str(body);
    tokio::fs::write(&abs_path, content).await?;
    Ok(())
}

async fn write_ydoc_to_disk(
    slug: &str,
    doc: &NoteDoc,
    site_config: &monowiki_core::Config,
) -> Result<()> {
    let rel_path = slug_to_rel(slug)?;
    let mut ydoc_rel = PathBuf::from(".collab").join(rel_path);
    ydoc_rel.set_extension("ydoc");
    let abs_path = site_config.vault_dir().join(&ydoc_rel);
    if let Some(parent) = abs_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let update = {
        let txn = doc.awareness.doc().transact();
        txn.encode_state_as_update_v1(&StateVector::default())
    };
    tokio::fs::write(&abs_path, update).await?;
    Ok(())
}

const TEXT_FIELD: &str = "body";
