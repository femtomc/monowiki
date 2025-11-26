//! Actor model primitives
//!
//! This module provides the core actor abstractions:
//! - `Actor`: An independent computational entity
//! - `Facet`: A unit of isolation within an actor
//! - `Entity`: Trait for implementing actor behaviors
//! - `EntityContext`: Controlled access for entity callbacks

mod entity;
mod facet;

pub use entity::{CollectingEntity, Entity, EntityContext, LoggingEntity, NoopEntity};
pub use facet::Facet;

use crate::dataspace::{Dataspace, DataspaceRef};
use crate::types::{ActorId, FacetId, Handle};
use std::collections::HashMap;

/// An actor in the syndicated actor model
///
/// Actors are independent computational entities that:
/// - Have a unique identity
/// - Contain facets (units of isolation)
/// - Hold capabilities to dataspaces
/// - Host entities that implement behavior
pub struct Actor<D: Dataspace> {
    /// Unique actor identifier
    id: ActorId,
    /// Actor's facets
    facets: HashMap<FacetId, Facet>,
    /// The root facet
    root_facet_id: FacetId,
    /// Capabilities to dataspaces, keyed by dataspace name
    capabilities: HashMap<String, DataspaceRef<D>>,
    /// Handles asserted by this actor (for cleanup on stop)
    asserted_handles: HashMap<String, Vec<Handle>>,
}

impl<D: Dataspace> Actor<D> {
    /// Create a new actor with the given ID
    pub fn new(id: ActorId) -> Self {
        let root_facet_id = FacetId::root(id.clone());
        let root_facet = Facet::root(root_facet_id.clone());

        let mut facets = HashMap::new();
        facets.insert(root_facet_id.clone(), root_facet);

        Self {
            id,
            facets,
            root_facet_id,
            capabilities: HashMap::new(),
            asserted_handles: HashMap::new(),
        }
    }

    /// Get the actor ID
    pub fn id(&self) -> &ActorId {
        &self.id
    }

    /// Get the root facet ID
    pub fn root_facet(&self) -> &FacetId {
        &self.root_facet_id
    }

    /// Get a facet by ID
    pub fn facet(&self, id: &FacetId) -> Option<&Facet> {
        self.facets.get(id)
    }

    /// Get a mutable facet by ID
    pub fn facet_mut(&mut self, id: &FacetId) -> Option<&mut Facet> {
        self.facets.get_mut(id)
    }

    /// Spawn a new facet as a child of the given parent
    pub fn spawn_facet(&mut self, parent: &FacetId) -> Option<FacetId> {
        if !self.facets.contains_key(parent) {
            return None;
        }

        let new_id = FacetId::new(self.id.clone());
        let new_facet = Facet::new(new_id.clone(), Some(parent.clone()));
        self.facets.insert(new_id.clone(), new_facet);

        // Update parent's children
        if let Some(parent_facet) = self.facets.get_mut(parent) {
            parent_facet.add_child(new_id.clone());
        }

        Some(new_id)
    }

    /// Stop a facet and all its children
    ///
    /// When a facet stops:
    /// - All its child facets are stopped recursively
    /// - All assertions made through the facet are retracted
    /// - All entities on the facet receive on_stop
    pub fn stop_facet(&mut self, facet_id: &FacetId) -> bool {
        // Collect children to stop first
        let children: Vec<FacetId> = self
            .facets
            .get(facet_id)
            .map(|f| f.children().to_vec())
            .unwrap_or_default();

        // Stop children recursively
        for child in children {
            self.stop_facet(&child);
        }

        // Remove the facet
        if let Some(mut facet) = self.facets.remove(facet_id) {
            facet.mark_stopped();

            // Remove from parent's children
            if let Some(parent_id) = facet.parent() {
                if let Some(parent) = self.facets.get_mut(parent_id) {
                    parent.remove_child(facet_id);
                }
            }

            true
        } else {
            false
        }
    }

    /// Grant a capability to this actor
    pub fn grant_capability(&mut self, dataspace_name: String, capability: DataspaceRef<D>) {
        self.capabilities.insert(dataspace_name, capability);
    }

    /// Get a capability by dataspace name
    pub fn capability(&self, name: &str) -> Option<&DataspaceRef<D>> {
        self.capabilities.get(name)
    }

    /// Get a mutable capability by dataspace name
    pub fn capability_mut(&mut self, name: &str) -> Option<&mut DataspaceRef<D>> {
        self.capabilities.get_mut(name)
    }

    /// List all capability names
    pub fn capability_names(&self) -> impl Iterator<Item = &String> {
        self.capabilities.keys()
    }

    /// Check if the actor is alive (has an active root facet)
    pub fn is_alive(&self) -> bool {
        self.facets
            .get(&self.root_facet_id)
            .map(|f| f.is_active())
            .unwrap_or(false)
    }

    /// Stop the entire actor
    pub fn stop(&mut self) {
        self.stop_facet(&self.root_facet_id.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataspace::{LocalDataspace, Permissions};

    type TestDataspace = LocalDataspace;

    #[test]
    fn test_actor_creation() {
        let actor: Actor<TestDataspace> = Actor::new(ActorId::new("test"));
        assert_eq!(actor.id().name(), "test");
        assert!(actor.is_alive());
    }

    #[test]
    fn test_spawn_facet() {
        let mut actor: Actor<TestDataspace> = Actor::new(ActorId::new("test"));
        let root = actor.root_facet().clone();

        let child = actor.spawn_facet(&root).expect("should spawn");
        assert!(actor.facet(&child).is_some());
        assert!(!child.is_root());
    }

    #[test]
    fn test_stop_facet() {
        let mut actor: Actor<TestDataspace> = Actor::new(ActorId::new("test"));
        let root = actor.root_facet().clone();

        let child = actor.spawn_facet(&root).expect("should spawn");
        let grandchild = actor.spawn_facet(&child).expect("should spawn");

        // Stop the child - should also stop grandchild
        actor.stop_facet(&child);

        assert!(actor.facet(&child).is_none());
        assert!(actor.facet(&grandchild).is_none());
        assert!(actor.facet(&root).is_some()); // Root should still exist
    }

    #[test]
    fn test_stop_actor() {
        let mut actor: Actor<TestDataspace> = Actor::new(ActorId::new("test"));
        assert!(actor.is_alive());

        actor.stop();
        assert!(!actor.is_alive());
    }

    #[test]
    fn test_grant_capability() {
        use parking_lot::RwLock;
        use std::sync::Arc;

        let mut actor: Actor<TestDataspace> = Actor::new(ActorId::new("test"));

        let ds = Arc::new(RwLock::new(LocalDataspace::new("my-ds")));
        let cap = DataspaceRef::new("my-ds", ds, Permissions::default());

        actor.grant_capability("my-ds".to_string(), cap);

        assert!(actor.capability("my-ds").is_some());
        assert!(actor.capability("other").is_none());
    }
}
