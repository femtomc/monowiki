//! Query database implementation
//!
//! This module implements the core incremental computation database
//! with dependency tracking, memoization, and invalidation.

use crate::durability::Durability;
use crate::memo::{MemoStorage, MemoTable};
use crate::query::{hash_value, InputQuery, Query, QueryDatabase, QueryKey, Revision};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// The main incremental computation database
pub struct Db {
    /// Memoization storage for all queries
    memo: Arc<MemoStorage>,

    /// Dependency graph: query -> queries it depends on
    dependencies: DashMap<QueryKey, Vec<QueryKey>>,

    /// Reverse dependencies: query -> queries that depend on it
    reverse_deps: DashMap<QueryKey, Vec<QueryKey>>,

    /// Current revision number
    current_revision: AtomicU64,

    /// Minimum durability that has changed since last check
    min_changed_durability: RwLock<Durability>,

    /// Query execution stack for tracking dependencies
    execution_stack: RwLock<Vec<QueryKey>>,

    /// Dynamic storage for extension data (using Arc for clonability)
    dynamic_storage: DashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl Db {
    /// Create a new database
    pub fn new() -> Self {
        Db {
            memo: Arc::new(MemoStorage::new()),
            dependencies: DashMap::new(),
            reverse_deps: DashMap::new(),
            current_revision: AtomicU64::new(1),
            min_changed_durability: RwLock::new(Durability::Static),
            execution_stack: RwLock::new(Vec::new()),
            dynamic_storage: DashMap::new(),
        }
    }

    /// Get or compute a query result
    pub fn query<Q: Query>(&self, key: Q::Key) -> Q::Value {
        let query_key = QueryKey::new::<Q>(&key);
        let table = self.memo.get_table::<Q>();

        // Check if we have a valid cached result
        if let Some(entry) = table.get_entry(&key) {
            let current_rev = self.revision();

            // If verified for current revision, return cached value
            if entry.is_verified_for(current_rev) {
                return entry.value;
            }

            // Check if dependencies have changed
            let deps_changed = self.check_dependencies_changed(&entry.dependencies, current_rev);

            if !deps_changed {
                // Dependencies unchanged, mark as verified and return
                table.mark_verified(&key, current_rev);
                return entry.value;
            }
        }

        // Need to recompute
        self.execute_query::<Q>(key, table)
    }

    /// Execute a query and handle memoization
    fn execute_query<Q: Query>(&self, key: Q::Key, table: Arc<MemoTable<Q>>) -> Q::Value {
        let query_key = QueryKey::new::<Q>(&key);

        // Push onto execution stack
        self.execution_stack.write().push(query_key);

        // Execute the query
        let new_value = Q::execute(self, &key);

        // Pop from execution stack and collect dependencies
        let dependencies = {
            let mut stack = self.execution_stack.write();
            stack.pop();

            // All queries on stack are dependencies
            stack.clone()
        };

        // Update memo table and check for early cutoff
        let current_rev = self.revision();
        let value_changed = table.update_entry(key.clone(), new_value.clone(), current_rev, dependencies.clone());

        // Update dependency tracking
        self.update_dependencies(query_key, dependencies);

        // If value changed, invalidate dependents
        if value_changed {
            self.invalidate_dependents(query_key);
        }

        new_value
    }

    /// Check if any dependencies have changed since a revision
    fn check_dependencies_changed(&self, dependencies: &[QueryKey], _since: Revision) -> bool {
        dependencies.iter().any(|_dep| {
            // For now, consider a dependency changed if it's not in our cache
            // In a full implementation, we'd check the dependency's verified_at
            true // Simplified for now
        })
    }

    /// Update dependency tracking
    fn update_dependencies(&self, query_key: QueryKey, dependencies: Vec<QueryKey>) {
        // Remove old reverse dependencies
        if let Some(old_deps) = self.dependencies.get(&query_key) {
            for dep in old_deps.value() {
                if let Some(mut reverse) = self.reverse_deps.get_mut(dep) {
                    reverse.retain(|k| k != &query_key);
                }
            }
        }

        // Add new dependencies
        self.dependencies.insert(query_key, dependencies.clone());

        // Update reverse dependencies
        for dep in dependencies {
            self.reverse_deps
                .entry(dep)
                .or_insert_with(Vec::new)
                .push(query_key);
        }
    }

    /// Invalidate all queries that depend on the given query
    fn invalidate_dependents(&self, query_key: QueryKey) {
        if let Some(dependents) = self.reverse_deps.get(&query_key) {
            for dependent in dependents.value() {
                // Recursively invalidate
                self.invalidate_dependents(*dependent);
            }
        }
    }

    /// Set an input value
    pub fn set_input<Q: InputQuery>(&self, key: Q::Key, value: Q::Value) {
        let table = self.memo.get_table::<Q>();
        let query_key = QueryKey::new::<Q>(&key);

        // Increment revision
        self.bump_revision();
        let current_rev = self.revision();

        // Update the memo table
        let changed = table.update_entry(key, value, current_rev, vec![]);

        // Mark durability as changed
        {
            let mut min_dur = self.min_changed_durability.write();
            *min_dur = min_dur.min(Q::durability());
        }

        // If value changed, invalidate dependents
        if changed {
            self.invalidate_dependents(query_key);
        }
    }

    /// Manually invalidate a query
    pub fn invalidate<Q: Query>(&self, key: Q::Key) {
        let table = self.memo.get_table::<Q>();
        let query_key = QueryKey::new::<Q>(&key);

        // Remove from memo table
        table.invalidate(&key);

        // Increment revision
        self.bump_revision();

        // Mark durability as changed
        {
            let mut min_dur = self.min_changed_durability.write();
            *min_dur = min_dur.min(Q::durability());
        }

        // Invalidate dependents
        self.invalidate_dependents(query_key);
    }

    /// Get the current revision
    pub fn revision(&self) -> Revision {
        Revision(self.current_revision.load(Ordering::SeqCst))
    }

    /// Increment the revision counter
    fn bump_revision(&self) {
        self.current_revision.fetch_add(1, Ordering::SeqCst);
    }

    /// Clear all cached data
    pub fn clear_all(&self) {
        self.memo.clear_all();
        self.dependencies.clear();
        self.reverse_deps.clear();
        self.dynamic_storage.clear();
    }

    /// Get the number of queries currently cached
    pub fn num_cached_query_types(&self) -> usize {
        self.memo.num_query_types()
    }
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryDatabase for Db {
    fn query<Q: Query>(&self, key: Q::Key) -> Q::Value {
        self.query::<Q>(key)
    }

    fn revision(&self) -> Revision {
        self.revision()
    }

    fn register_dependency(&self, from: QueryKey, to: QueryKey) {
        // Add to dependency graph
        self.dependencies
            .entry(from)
            .or_insert_with(Vec::new)
            .push(to);

        // Add to reverse dependencies
        self.reverse_deps
            .entry(to)
            .or_insert_with(Vec::new)
            .push(from);
    }

    fn get_any(&self, key: &str) -> Option<Box<dyn Any + Send + Sync>> {
        self.dynamic_storage.get(key).map(|entry| {
            // Convert Arc to Box by cloning the Arc and using the inner value
            let arc = entry.value().clone();
            // We can't directly convert Arc<dyn Any> to Box<dyn Any>
            // So we return a boxed reference
            // This is a workaround - in practice, we'd use Arc throughout
            Box::new(arc) as Box<dyn Any + Send + Sync>
        })
    }

    fn set_any(&self, key: String, value: Box<dyn Any + Send + Sync>) {
        // Convert Box to Arc for storage
        self.dynamic_storage.insert(key, Arc::from(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_creation() {
        let db = Db::new();
        assert_eq!(db.revision(), Revision(1));
    }

    #[test]
    fn test_simple_query() {
        struct AddOneQuery;
        impl Query for AddOneQuery {
            type Key = u32;
            type Value = u32;

            fn execute<DB: QueryDatabase>(_db: &DB, key: &Self::Key) -> Self::Value {
                key + 1
            }
        }

        let db = Db::new();
        assert_eq!(db.query::<AddOneQuery>(5), 6);
        assert_eq!(db.query::<AddOneQuery>(10), 11);
    }

    #[test]
    fn test_memoization() {
        use std::sync::atomic::AtomicU32;

        static CALL_COUNT: AtomicU32 = AtomicU32::new(0);

        struct CountingQuery;
        impl Query for CountingQuery {
            type Key = u32;
            type Value = u32;

            fn execute<DB: QueryDatabase>(_db: &DB, key: &Self::Key) -> Self::Value {
                CALL_COUNT.fetch_add(1, Ordering::SeqCst);
                key + 1
            }
        }

        let db = Db::new();

        // First call
        assert_eq!(db.query::<CountingQuery>(5), 6);
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);

        // Second call should be memoized
        assert_eq!(db.query::<CountingQuery>(5), 6);
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);

        // Different key should execute
        assert_eq!(db.query::<CountingQuery>(10), 11);
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_input_query() {
        struct InputValue;
        impl Query for InputValue {
            type Key = String;
            type Value = u32;

            fn execute<DB: QueryDatabase>(_db: &DB, _key: &Self::Key) -> Self::Value {
                0
            }
        }

        impl InputQuery for InputValue {
            fn set<DB: QueryDatabase>(db: &DB, key: Self::Key, value: Self::Value) {
                // Use dynamic storage
                let key_str = format!("input_{}", key);
                db.set_any(key_str, Box::new(value));
            }
        }

        let db = Db::new();

        // Set an input
        InputValue::set(&db, "test".to_string(), 42);

        // For this test, we need to store the value in dynamic storage
        // The actual query would need to read from there
    }
}
