//! Content-addressable cache for query results
//!
//! This module provides persistent caching of query results based on
//! content hashes, enabling cross-session caching and efficient reuse.

use blake3::Hasher;
use dashmap::DashMap;
use serde::{de::DeserializeOwned, Serialize};
use std::time::{Duration, Instant};

/// A cache key based on content hash
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Blake3 hash of the cache key components
    pub content_hash: [u8; 32],
}

impl CacheKey {
    /// Create a cache key from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(bytes);
        CacheKey {
            content_hash: *hasher.finalize().as_bytes(),
        }
    }

    /// Create a cache key from multiple components
    pub fn from_components(components: &[&[u8]]) -> Self {
        let mut hasher = Hasher::new();
        for component in components {
            hasher.update(component);
        }
        CacheKey {
            content_hash: *hasher.finalize().as_bytes(),
        }
    }

    /// Create a cache key for expansion
    ///
    /// This combines source text, macro version, and config hash
    pub fn for_expand(source: &str, macro_version: u64, config_hash: u64) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(source.as_bytes());
        hasher.update(&macro_version.to_le_bytes());
        hasher.update(&config_hash.to_le_bytes());
        CacheKey {
            content_hash: *hasher.finalize().as_bytes(),
        }
    }

    /// Create a cache key for layout
    ///
    /// This combines content hash, style hash, and viewport hash
    pub fn for_layout(content_hash: u64, style_hash: u64, viewport_hash: u64) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(&content_hash.to_le_bytes());
        hasher.update(&style_hash.to_le_bytes());
        hasher.update(&viewport_hash.to_le_bytes());
        CacheKey {
            content_hash: *hasher.finalize().as_bytes(),
        }
    }

    /// Get the hash as a hex string
    pub fn to_hex(&self) -> String {
        hex::encode(self.content_hash)
    }
}

/// A cached entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Serialized value
    pub value: Vec<u8>,

    /// When this entry was created
    pub created_at: Instant,

    /// Number of times accessed
    pub access_count: u64,

    /// Last access time
    pub last_accessed: Instant,
}

impl CacheEntry {
    /// Create a new cache entry
    pub fn new(value: Vec<u8>) -> Self {
        let now = Instant::now();
        CacheEntry {
            value,
            created_at: now,
            access_count: 0,
            last_accessed: now,
        }
    }

    /// Record an access
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Instant::now();
    }

    /// Get the age of this entry
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Get time since last access
    pub fn idle_time(&self) -> Duration {
        self.last_accessed.elapsed()
    }
}

/// Statistics about cache usage
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
    pub total_size_bytes: usize,
}

impl CacheStats {
    /// Get cache hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl std::fmt::Display for CacheStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Cache Statistics:")?;
        writeln!(
            f,
            "  Hits: {} | Misses: {} | Hit Rate: {:.1}%",
            self.hits,
            self.misses,
            self.hit_rate() * 100.0
        )?;
        writeln!(f, "  Entries: {}", self.entries)?;
        writeln!(
            f,
            "  Total Size: {:.2} MB",
            self.total_size_bytes as f64 / 1_048_576.0
        )?;
        Ok(())
    }
}

/// In-memory content-addressable cache
pub struct ContentCache {
    /// Cache entries
    entries: DashMap<CacheKey, CacheEntry>,

    /// Cache statistics
    hits: std::sync::atomic::AtomicU64,
    misses: std::sync::atomic::AtomicU64,

    /// Maximum cache size in bytes (0 = unlimited)
    max_size_bytes: usize,
}

impl ContentCache {
    /// Create a new content cache
    pub fn new() -> Self {
        ContentCache {
            entries: DashMap::new(),
            hits: std::sync::atomic::AtomicU64::new(0),
            misses: std::sync::atomic::AtomicU64::new(0),
            max_size_bytes: 0,
        }
    }

    /// Create a new cache with a size limit
    pub fn with_max_size(max_size_bytes: usize) -> Self {
        ContentCache {
            entries: DashMap::new(),
            hits: std::sync::atomic::AtomicU64::new(0),
            misses: std::sync::atomic::AtomicU64::new(0),
            max_size_bytes,
        }
    }

    /// Get a value from the cache
    pub fn get<T: DeserializeOwned>(&self, key: &CacheKey) -> Option<T> {
        if let Some(mut entry) = self.entries.get_mut(key) {
            entry.record_access();
            self.hits
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Deserialize the value
            serde_json::from_slice(&entry.value).ok()
        } else {
            self.misses
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            None
        }
    }

    /// Put a value in the cache
    pub fn put<T: Serialize>(&self, key: CacheKey, value: &T) -> Result<(), CacheError> {
        // Serialize the value
        let serialized = serde_json::to_vec(value)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;

        // Check size limit
        if self.max_size_bytes > 0 {
            let current_size = self.total_size();
            if current_size + serialized.len() > self.max_size_bytes {
                // Evict oldest entries
                self.evict_to_fit(serialized.len());
            }
        }

        // Insert the entry
        self.entries.insert(key, CacheEntry::new(serialized));

        Ok(())
    }

    /// Remove an entry from the cache
    pub fn remove(&self, key: &CacheKey) -> bool {
        self.entries.remove(key).is_some()
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.clear();
        self.hits.store(0, std::sync::atomic::Ordering::Relaxed);
        self.misses
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total_size = self.total_size();

        CacheStats {
            hits: self.hits.load(std::sync::atomic::Ordering::Relaxed),
            misses: self.misses.load(std::sync::atomic::Ordering::Relaxed),
            entries: self.entries.len(),
            total_size_bytes: total_size,
        }
    }

    /// Get total size of cached data
    fn total_size(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| entry.value.len())
            .sum()
    }

    /// Evict entries to make room for new data
    fn evict_to_fit(&self, needed_bytes: usize) {
        // Simple LRU eviction: remove entries by last access time
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .map(|e| (e.key().clone(), e.value().idle_time()))
            .collect();

        // Sort by idle time (oldest first)
        entries.sort_by_key(|(_, idle)| std::cmp::Reverse(*idle));

        let mut freed = 0;
        for (key, _) in entries {
            if freed >= needed_bytes {
                break;
            }

            if let Some((_, entry)) = self.entries.remove(&key) {
                freed += entry.value.len();
            }
        }
    }

    /// Evict entries older than the given duration
    pub fn evict_older_than(&self, max_age: Duration) {
        let keys_to_remove: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.value().age() > max_age)
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.entries.remove(&key);
        }
    }

    /// Evict entries not accessed for the given duration
    pub fn evict_idle(&self, max_idle: Duration) {
        let keys_to_remove: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.value().idle_time() > max_idle)
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            self.entries.remove(&key);
        }
    }
}

impl Default for ContentCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during caching
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Failed to serialize value: {0}")]
    SerializationError(String),

    #[error("Failed to deserialize value: {0}")]
    DeserializationError(String),

    #[error("Cache size limit exceeded")]
    SizeLimitExceeded,
}

// Note: We need to add hex crate to Cargo.toml for to_hex()
// For now, we'll implement a simple hex encoder

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_creation() {
        let key1 = CacheKey::from_bytes(b"test");
        let key2 = CacheKey::from_bytes(b"test");
        let key3 = CacheKey::from_bytes(b"different");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_get_put() {
        let cache = ContentCache::new();

        let key = CacheKey::from_bytes(b"test_key");
        let value = "test_value".to_string();

        // Put a value
        cache.put(key.clone(), &value).unwrap();

        // Get it back
        let retrieved: Option<String> = cache.get(&key);
        assert_eq!(retrieved, Some(value));
    }

    #[test]
    fn test_cache_miss() {
        let cache = ContentCache::new();

        let key = CacheKey::from_bytes(b"nonexistent");
        let retrieved: Option<String> = cache.get(&key);

        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_cache_stats() {
        let cache = ContentCache::new();

        let key = CacheKey::from_bytes(b"test");
        cache.put(key.clone(), &"value".to_string()).unwrap();

        // One hit
        let _: Option<String> = cache.get(&key);

        // One miss
        let other_key = CacheKey::from_bytes(b"other");
        let _: Option<String> = cache.get(&other_key);

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn test_cache_clear() {
        let cache = ContentCache::new();

        let key = CacheKey::from_bytes(b"test");
        cache.put(key, &"value".to_string()).unwrap();

        cache.clear();

        let stats = cache.stats();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
    }
}
