//! Core type definitions for the Sammy runtime
//!
//! These are the foundational identity and handle types used throughout
//! the syndicated actor runtime.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

/// Assertion handle for tracking and retracting assertions
///
/// Handles are unforgeable identifiers that allow the asserter to
/// later retract their assertion. Each handle is globally unique.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Handle(pub Uuid);

impl Handle {
    /// Create a new unique handle
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a handle from an existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for Handle {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "h:{}", &self.0.to_string()[..8])
    }
}

/// Actor identifier
///
/// Each actor in the runtime has a unique identifier. Actor IDs are
/// typically human-readable names for debugging purposes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub String);

impl ActorId {
    /// Create a new actor ID with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the actor name
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Facet identifier within an actor
///
/// Facets are units of isolation within an actor. When a facet stops,
/// all assertions made through that facet are automatically retracted.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FacetId {
    /// The actor this facet belongs to
    pub actor: ActorId,
    /// Unique identifier within the actor
    pub id: u64,
}

static FACET_COUNTER: AtomicU64 = AtomicU64::new(1);

impl FacetId {
    /// Create a new facet ID for the given actor
    pub fn new(actor: ActorId) -> Self {
        Self {
            actor,
            id: FACET_COUNTER.fetch_add(1, Ordering::SeqCst),
        }
    }

    /// Create the root facet for an actor
    pub fn root(actor: ActorId) -> Self {
        Self { actor, id: 0 }
    }

    /// Check if this is the root facet
    pub fn is_root(&self) -> bool {
        self.id == 0
    }
}

impl fmt::Display for FacetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.actor, self.id)
    }
}

/// Subscription identifier
///
/// Returned when subscribing to a pattern in a dataspace,
/// used to later unsubscribe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubscriptionId(pub u64);

static SUBSCRIPTION_COUNTER: AtomicU64 = AtomicU64::new(1);

impl SubscriptionId {
    /// Create a new unique subscription ID
    pub fn new() -> Self {
        Self(SUBSCRIPTION_COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for SubscriptionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sub:{}", self.0)
    }
}

/// Turn identifier for causality tracking
///
/// Each turn in the runtime is assigned a unique, monotonically
/// increasing identifier for ordering and replay.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TurnId(pub u64);

static TURN_COUNTER: AtomicU64 = AtomicU64::new(1);

impl TurnId {
    /// Create a new turn ID
    pub fn new() -> Self {
        Self(TURN_COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    /// The genesis turn (before any computation)
    pub fn genesis() -> Self {
        Self(0)
    }
}

impl Default for TurnId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TurnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "turn:{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_uniqueness() {
        let h1 = Handle::new();
        let h2 = Handle::new();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_facet_root() {
        let actor = ActorId::new("test");
        let root = FacetId::root(actor.clone());
        assert!(root.is_root());

        let child = FacetId::new(actor);
        assert!(!child.is_root());
    }

    #[test]
    fn test_subscription_id_uniqueness() {
        let s1 = SubscriptionId::new();
        let s2 = SubscriptionId::new();
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_turn_ordering() {
        let t1 = TurnId::new();
        let t2 = TurnId::new();
        assert!(t1 < t2);
    }
}
