//! Memoization with early cutoff
//!
//! This module implements the memoization layer that caches query results
//! and performs early cutoff when recomputed values are unchanged.

use crate::durability::Durability;
use crate::query::{hash_value, Query, QueryDatabase, QueryKey, Revision};
use dashmap::DashMap;
use std::any::TypeId;
use std::hash::Hash;
use std::sync::Arc;

/// A memoized entry for a query result
#[derive(Debug, Clone)]
pub struct MemoEntry<V> {
    /// The cached value
    pub value: V,

    /// Hash of the value for early cutoff
    pub value_hash: u64,

    /// Revision when this was computed
    pub computed_at: Revision,

    /// Revision when this was last verified as still valid
    pub verified_at: Revision,

    /// Dependencies of this query
    pub dependencies: Vec<QueryKey>,

    /// Durability of this entry
    pub durability: Durability,
}

impl<V: Hash> MemoEntry<V> {
    /// Create a new memo entry
    pub fn new(
        value: V,
        computed_at: Revision,
        dependencies: Vec<QueryKey>,
        durability: Durability,
    ) -> Self {
        let value_hash = hash_value(&value);

        MemoEntry {
            value,
            value_hash,
            computed_at,
            verified_at: computed_at,
            dependencies,
            durability,
        }
    }

    /// Check if a new value is unchanged (early cutoff check)
    pub fn is_unchanged(&self, new_value: &V) -> bool {
        let new_hash = hash_value(new_value);
        new_hash == self.value_hash
    }

    /// Update verified_at timestamp
    pub fn mark_verified(&mut self, revision: Revision) {
        self.verified_at = revision;
    }

    /// Check if this entry is verified for the given revision
    pub fn is_verified_for(&self, revision: Revision) -> bool {
        self.verified_at >= revision
    }
}

/// Memoization table for a specific query type
pub struct MemoTable<Q: Query> {
    /// Map from key to memo entry
    entries: DashMap<Q::Key, MemoEntry<Q::Value>>,

    /// Type ID for this query (for debugging)
    query_type: TypeId,
}

impl<Q: Query> MemoTable<Q> {
    /// Create a new memo table
    pub fn new() -> Self {
        MemoTable {
            entries: DashMap::new(),
            query_type: TypeId::of::<Q>(),
        }
    }

    /// Get a cached value if it exists and is valid
    pub fn get(&self, key: &Q::Key) -> Option<Q::Value> {
        self.entries.get(key).map(|entry| entry.value.clone())
    }

    /// Get the full memo entry
    pub fn get_entry(&self, key: &Q::Key) -> Option<MemoEntry<Q::Value>> {
        self.entries.get(key).map(|entry| entry.clone())
    }

    /// Insert or update a memo entry
    pub fn insert(
        &self,
        key: Q::Key,
        value: Q::Value,
        computed_at: Revision,
        dependencies: Vec<QueryKey>,
        durability: Durability,
    ) -> Option<MemoEntry<Q::Value>> {
        let entry = MemoEntry::new(value, computed_at, dependencies, durability);
        self.entries.insert(key, entry)
    }

    /// Update an existing entry with a new value
    ///
    /// Returns true if the value changed (early cutoff failed)
    pub fn update_entry(
        &self,
        key: Q::Key,
        new_value: Q::Value,
        computed_at: Revision,
        dependencies: Vec<QueryKey>,
    ) -> bool {
        let new_hash = hash_value(&new_value);

        // Try to update existing entry
        if let Some(mut entry) = self.entries.get_mut(&key) {
            let changed = entry.value_hash != new_hash;

            // Update the entry
            entry.value = new_value;
            entry.value_hash = new_hash;
            entry.computed_at = computed_at;
            entry.verified_at = computed_at;
            entry.dependencies = dependencies;

            changed
        } else {
            // No existing entry, insert new one
            self.insert(key, new_value, computed_at, dependencies, Q::durability());
            true
        }
    }

    /// Mark an entry as verified for the given revision
    pub fn mark_verified(&self, key: &Q::Key, revision: Revision) {
        if let Some(mut entry) = self.entries.get_mut(key) {
            entry.mark_verified(revision);
        }
    }

    /// Invalidate an entry (remove it)
    pub fn invalidate(&self, key: &Q::Key) -> Option<MemoEntry<Q::Value>> {
        self.entries.remove(key).map(|(_, v)| v)
    }

    /// Clear all entries
    pub fn clear(&self) {
        self.entries.clear();
    }

    /// Get the number of cached entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the table is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all keys currently cached
    pub fn keys(&self) -> Vec<Q::Key> {
        self.entries.iter().map(|entry| entry.key().clone()).collect()
    }
}

impl<Q: Query> Default for MemoTable<Q> {
    fn default() -> Self {
        Self::new()
    }
}

/// Storage for all memo tables
///
/// This structure holds the memo tables for all query types.
pub struct MemoStorage {
    tables: DashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl MemoStorage {
    /// Create a new memo storage
    pub fn new() -> Self {
        MemoStorage {
            tables: DashMap::new(),
        }
    }

    /// Get or create a memo table for a query type
    pub fn get_table<Q: Query>(&self) -> Arc<MemoTable<Q>> {
        let type_id = TypeId::of::<Q>();

        if let Some(table) = self.tables.get(&type_id) {
            // SAFETY: We only insert MemoTable<Q> for TypeId::of::<Q>()
            return table
                .value()
                .clone()
                .downcast::<MemoTable<Q>>()
                .expect("type mismatch in memo storage");
        }

        // Create new table
        let table = Arc::new(MemoTable::<Q>::new());
        self.tables.insert(type_id, table.clone());
        table
    }

    /// Clear all memo tables
    pub fn clear_all(&self) {
        self.tables.clear();
    }

    /// Get the number of query types cached
    pub fn num_query_types(&self) -> usize {
        self.tables.len()
    }
}

impl Default for MemoStorage {
    fn default() -> Self {
        Self::new()
    }
}

// Import Any trait for downcasting
use std::any::Any;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct TestValue(String);

    #[test]
    fn test_memo_entry_unchanged() {
        let value = TestValue("test".to_string());
        let entry = MemoEntry::new(value.clone(), Revision(1), vec![], Durability::Volatile);

        assert!(entry.is_unchanged(&value));
        assert!(!entry.is_unchanged(&TestValue("different".to_string())));
    }

    #[test]
    fn test_memo_table_basic() {
        struct DummyQuery;
        impl Query for DummyQuery {
            type Key = u32;
            type Value = String;

            fn execute<DB: QueryDatabase>(_db: &DB, _key: &Self::Key) -> Self::Value {
                String::new()
            }
        }

        let table = MemoTable::<DummyQuery>::new();

        // Insert a value
        table.insert(
            42,
            "test".to_string(),
            Revision(1),
            vec![],
            Durability::Volatile,
        );

        // Retrieve it
        assert_eq!(table.get(&42), Some("test".to_string()));
        assert_eq!(table.get(&43), None);
    }

    #[test]
    fn test_memo_table_update() {
        struct DummyQuery;
        impl Query for DummyQuery {
            type Key = u32;
            type Value = String;

            fn execute<DB: QueryDatabase>(_db: &DB, _key: &Self::Key) -> Self::Value {
                String::new()
            }
        }

        let table = MemoTable::<DummyQuery>::new();

        // Insert initial value
        table.insert(
            42,
            "test".to_string(),
            Revision(1),
            vec![],
            Durability::Volatile,
        );

        // Update with same value (early cutoff)
        let changed = table.update_entry(42, "test".to_string(), Revision(2), vec![]);
        assert!(!changed);

        // Update with different value
        let changed = table.update_entry(42, "new".to_string(), Revision(3), vec![]);
        assert!(changed);
    }
}
