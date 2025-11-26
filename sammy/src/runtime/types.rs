//! Core type definitions for the Sammy runtime

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Actor identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub String);

impl ActorId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Facet identifier within an actor
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FacetId(pub String);

impl FacetId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for FacetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Assertion handle for tracking and retracting assertions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Handle(pub Uuid);

impl Handle {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for Handle {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Turn identifier for causality tracking
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TurnId(pub String);

impl TurnId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn genesis() -> Self {
        Self("genesis".to_string())
    }
}

impl fmt::Display for TurnId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Branch identifier for time-travel
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchId(pub String);

impl BranchId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn main() -> Self {
        Self("main".to_string())
    }
}

impl fmt::Display for BranchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_id() {
        let id = ActorId::new("test-actor");
        assert_eq!(id.0, "test-actor");
        assert_eq!(format!("{}", id), "test-actor");
    }

    #[test]
    fn test_handle_uniqueness() {
        let h1 = Handle::new();
        let h2 = Handle::new();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_turn_id_ordering() {
        let t1 = TurnId::new("turn_001");
        let t2 = TurnId::new("turn_002");
        assert!(t1 < t2);
    }
}
