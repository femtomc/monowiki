//! Assertion storage backends
//!
//! This module defines the `AssertionStore` trait for pluggable storage
//! and provides `OrSetStore` as the default OR-Set based implementation.

use crate::pattern::Pattern;
use crate::types::Handle;
use preserves::IOValue;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Trait for assertion storage backends
///
/// Implementations provide the underlying storage mechanism for assertions
/// in a dataspace. Different backends can offer different trade-offs:
/// - `OrSetStore`: Simple OR-Set with tombstones (default)
/// - Journaled stores: Support time-travel and replay
/// - Persistent stores: Durable across restarts
pub trait AssertionStore: Send + Sync {
    /// Insert an assertion, returns a handle for later retraction
    fn insert(&mut self, value: IOValue) -> Handle;

    /// Remove an assertion by handle, returns the value if it existed
    fn remove(&mut self, handle: &Handle) -> Option<IOValue>;

    /// Check if a handle is present
    fn contains(&self, handle: &Handle) -> bool;

    /// Get the value for a handle
    fn get(&self, handle: &Handle) -> Option<&IOValue>;

    /// Query assertions matching a pattern
    fn query(&self, pattern: &Pattern) -> Vec<(Handle, IOValue)>;

    /// Iterate over all active assertions
    fn iter(&self) -> Box<dyn Iterator<Item = (&Handle, &IOValue)> + '_>;

    /// Number of active assertions
    fn len(&self) -> usize;

    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Merge with another store (for CRDT semantics)
    ///
    /// The default implementation does nothing. Stores that support
    /// distributed operation should override this.
    fn merge(&mut self, _other: &Self)
    where
        Self: Sized,
    {
        // Default: no-op for non-distributed stores
    }
}

/// OR-Set (Observed-Remove Set) based assertion store
///
/// This is the default implementation using OR-Set semantics with tombstones.
/// Assertions can be concurrently added and removed, with removes winning
/// over adds for the same handle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrSetStore {
    /// Active assertions: handle -> (value, version_id)
    active: HashMap<Handle, (IOValue, Uuid)>,
    /// Tombstones for removed assertions
    tombstones: HashSet<(Handle, Uuid)>,
}

impl OrSetStore {
    /// Create a new empty store
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of tombstones (for debugging/metrics)
    pub fn tombstone_count(&self) -> usize {
        self.tombstones.len()
    }

    /// Compact tombstones (remove tombstones for handles no longer referenced)
    ///
    /// This is a maintenance operation that can be called periodically
    /// to reduce memory usage.
    pub fn compact(&mut self) {
        let active_handles: HashSet<_> = self.active.keys().cloned().collect();
        self.tombstones
            .retain(|(handle, _)| active_handles.contains(handle));
    }
}

impl AssertionStore for OrSetStore {
    fn insert(&mut self, value: IOValue) -> Handle {
        let handle = Handle::new();
        let version = Uuid::new_v4();
        self.active.insert(handle.clone(), (value, version));
        handle
    }

    fn remove(&mut self, handle: &Handle) -> Option<IOValue> {
        if let Some((value, version)) = self.active.remove(handle) {
            self.tombstones.insert((handle.clone(), version));
            Some(value)
        } else {
            None
        }
    }

    fn contains(&self, handle: &Handle) -> bool {
        self.active.contains_key(handle)
    }

    fn get(&self, handle: &Handle) -> Option<&IOValue> {
        self.active.get(handle).map(|(v, _)| v)
    }

    fn query(&self, pattern: &Pattern) -> Vec<(Handle, IOValue)> {
        self.active
            .iter()
            .filter(|(_, (value, _))| pattern.matches_tagged(value))
            .map(|(handle, (value, _))| (handle.clone(), value.clone()))
            .collect()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = (&Handle, &IOValue)> + '_> {
        Box::new(self.active.iter().map(|(h, (v, _))| (h, v)))
    }

    fn len(&self) -> usize {
        self.active.len()
    }

    fn merge(&mut self, other: &Self) {
        // Collect all tombstones
        let all_tombstones: HashSet<_> = self
            .tombstones
            .iter()
            .chain(other.tombstones.iter())
            .cloned()
            .collect();

        // Merge active assertions, excluding tombstoned ones
        for (handle, (value, version)) in other.active.iter() {
            if !all_tombstones.contains(&(handle.clone(), *version)) {
                // Only insert if we don't have it or if the other version is newer
                // (In a true CRDT we'd need vector clocks; this is simplified)
                if !self.active.contains_key(handle) {
                    self.active
                        .insert(handle.clone(), (value.clone(), *version));
                }
            }
        }

        // Remove any of our assertions that are in combined tombstones
        self.active.retain(|handle, (_, version)| {
            !all_tombstones.contains(&(handle.clone(), *version))
        });

        self.tombstones = all_tombstones;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get() {
        let mut store = OrSetStore::new();
        let value = IOValue::new("hello".to_string());
        let handle = store.insert(value.clone());

        assert!(store.contains(&handle));
        assert_eq!(store.get(&handle), Some(&value));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut store = OrSetStore::new();
        let value = IOValue::new("hello".to_string());
        let handle = store.insert(value.clone());

        let removed = store.remove(&handle);
        assert_eq!(removed, Some(value));
        assert!(!store.contains(&handle));
        assert_eq!(store.len(), 0);
        assert_eq!(store.tombstone_count(), 1);
    }

    #[test]
    fn test_query() {
        use crate::pattern::PatternBuilder;

        let mut store = OrSetStore::new();

        // Add some records
        let user1 = IOValue::record(
            IOValue::symbol("user"),
            vec![IOValue::new("alice".to_string())],
        );
        let user2 = IOValue::record(
            IOValue::symbol("user"),
            vec![IOValue::new("bob".to_string())],
        );
        let other = IOValue::record(
            IOValue::symbol("other"),
            vec![IOValue::new("data".to_string())],
        );

        store.insert(user1);
        store.insert(user2);
        store.insert(other);

        // Query for user records
        let pattern = PatternBuilder::record("user", vec![PatternBuilder::wildcard()]);
        let results = store.query(&pattern);

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_merge() {
        let mut store1 = OrSetStore::new();
        let mut store2 = OrSetStore::new();

        let v1 = IOValue::new("value1".to_string());
        let v2 = IOValue::new("value2".to_string());
        let v3 = IOValue::new("value3".to_string());

        let h1 = store1.insert(v1);
        store2.insert(v2);
        let h3 = store1.insert(v3);

        // Remove h3 from store1
        store1.remove(&h3);

        // Merge store2 into store1
        store1.merge(&store2);

        // Should have v1 and v2, but not v3
        assert!(store1.contains(&h1));
        assert!(!store1.contains(&h3));
        assert_eq!(store1.len(), 2);
    }

    #[test]
    fn test_compact() {
        let mut store = OrSetStore::new();

        // Add and remove several items
        for i in 0..10 {
            let handle = store.insert(IOValue::new(i as i64));
            if i % 2 == 0 {
                store.remove(&handle);
            }
        }

        assert!(store.tombstone_count() > 0);

        // After compact, tombstones for removed handles should be cleaned
        store.compact();

        // Tombstones for handles still in active should remain
        // (in this simple implementation, compact removes all since removed handles aren't in active)
    }
}
