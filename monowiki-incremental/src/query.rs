//! Query trait and key types
//!
//! This module defines the core Query trait that all incremental queries
//! must implement, along with supporting types for query keys and values.

use crate::durability::Durability;
use std::any::{Any, TypeId};
use std::fmt;
use std::hash::Hash;

/// Unique identifier for a query invocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryKey {
    /// Type ID of the query
    pub query_type: TypeId,

    /// Hash of the query's key
    pub key_hash: u64,
}

impl QueryKey {
    /// Create a new query key
    pub fn new<Q: Query>(key: &Q::Key) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);

        QueryKey {
            query_type: TypeId::of::<Q>(),
            key_hash: hasher.finish(),
        }
    }
}

impl fmt::Display for QueryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Query({:?}, {:016x})", self.query_type, self.key_hash)
    }
}

/// Revision number for tracking query freshness
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

impl Revision {
    pub const ZERO: Revision = Revision(0);

    pub fn next(self) -> Revision {
        Revision(self.0 + 1)
    }
}

impl Default for Revision {
    fn default() -> Self {
        Revision::ZERO
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

/// Core trait for incremental queries
///
/// Queries are pure, memoized functions that transform data.
/// They support dependency tracking and early cutoff.
pub trait Query: 'static + Send + Sync {
    /// The input key for this query
    type Key: Hash + Eq + Clone + Send + Sync + 'static;

    /// The output value produced by this query
    type Value: Clone + Hash + Send + Sync + 'static;

    /// Execute the query with the given database and key
    fn execute<DB: QueryDatabase>(db: &DB, key: &Self::Key) -> Self::Value;

    /// The durability tier of this query
    fn durability() -> Durability {
        Durability::Volatile
    }

    /// Optional name for debugging and metrics
    fn name() -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Trait for input queries that can be set directly
///
/// Input queries are the leaves of the dependency graph.
/// They represent values that come from outside the incremental system.
pub trait InputQuery: Query {
    /// Set the value for this input query
    fn set<DB: QueryDatabase>(db: &DB, key: Self::Key, value: Self::Value);
}

/// Database interface for queries
///
/// This trait provides the core operations that queries can perform
/// on the database during execution.
///
/// Note: This trait is implemented directly by Db, not used as a trait object.
/// Queries receive a reference to Db, not &dyn QueryDatabase.
pub trait QueryDatabase: Send + Sync + 'static {
    /// Query a value, tracking the dependency
    fn query<Q: Query>(&self, key: Q::Key) -> Q::Value;

    /// Get the current revision
    fn revision(&self) -> Revision;

    /// Register a dependency between queries
    fn register_dependency(&self, from: QueryKey, to: QueryKey);

    /// Get any value as dynamic (for extension)
    fn get_any(&self, key: &str) -> Option<Box<dyn Any + Send + Sync>>;

    /// Set any value as dynamic (for extension)
    fn set_any(&self, key: String, value: Box<dyn Any + Send + Sync>);
}

/// Helper for computing hash values
pub fn hash_value<T: Hash>(value: &T) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_key_equality() {
        struct TestQuery;
        impl Query for TestQuery {
            type Key = u32;
            type Value = String;

            fn execute<DB: QueryDatabase>(_db: &DB, _key: &Self::Key) -> Self::Value {
                String::new()
            }
        }

        let key1 = QueryKey::new::<TestQuery>(&42);
        let key2 = QueryKey::new::<TestQuery>(&42);
        let key3 = QueryKey::new::<TestQuery>(&43);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_revision_ordering() {
        let r1 = Revision(1);
        let r2 = Revision(2);

        assert!(r1 < r2);
        assert_eq!(r1.next(), r2);
    }
}
