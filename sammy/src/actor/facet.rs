//! Facet - unit of isolation within an actor
//!
//! Facets provide hierarchical isolation within an actor:
//! - When a facet stops, all its children stop
//! - Assertions made through a facet are tracked for cleanup
//! - Entities are registered on facets

use crate::types::{FacetId, Handle, SubscriptionId};
use std::collections::HashSet;

/// State of a facet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FacetState {
    /// The facet is active and can be used
    Active,
    /// The facet is stopping (cleanup in progress)
    Stopping,
    /// The facet has been stopped
    Stopped,
}

/// A facet is a unit of isolation within an actor
///
/// Facets form a tree structure. When a parent facet stops,
/// all its children are stopped first. This provides structured
/// concurrency and cleanup guarantees.
#[derive(Debug)]
pub struct Facet {
    /// Unique identifier for this facet
    id: FacetId,
    /// Parent facet (None for root)
    parent: Option<FacetId>,
    /// Child facets
    children: Vec<FacetId>,
    /// Current state
    state: FacetState,
    /// Handles asserted through this facet (for cleanup)
    handles: HashSet<Handle>,
    /// Active subscriptions (for cleanup)
    subscriptions: HashSet<SubscriptionId>,
}

impl Facet {
    /// Create a new facet with a parent
    pub fn new(id: FacetId, parent: Option<FacetId>) -> Self {
        Self {
            id,
            parent,
            children: Vec::new(),
            state: FacetState::Active,
            handles: HashSet::new(),
            subscriptions: HashSet::new(),
        }
    }

    /// Create the root facet for an actor
    pub fn root(id: FacetId) -> Self {
        Self::new(id, None)
    }

    /// Get the facet ID
    pub fn id(&self) -> &FacetId {
        &self.id
    }

    /// Get the parent facet ID
    pub fn parent(&self) -> Option<&FacetId> {
        self.parent.as_ref()
    }

    /// Check if this is the root facet
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Get child facet IDs
    pub fn children(&self) -> &[FacetId] {
        &self.children
    }

    /// Add a child facet
    pub fn add_child(&mut self, child: FacetId) {
        self.children.push(child);
    }

    /// Remove a child facet
    pub fn remove_child(&mut self, child: &FacetId) {
        self.children.retain(|c| c != child);
    }

    /// Get the current state
    pub fn state(&self) -> FacetState {
        self.state
    }

    /// Check if the facet is active
    pub fn is_active(&self) -> bool {
        self.state == FacetState::Active
    }

    /// Check if the facet is stopped
    pub fn is_stopped(&self) -> bool {
        self.state == FacetState::Stopped
    }

    /// Mark the facet as stopping
    pub fn mark_stopping(&mut self) {
        if self.state == FacetState::Active {
            self.state = FacetState::Stopping;
        }
    }

    /// Mark the facet as stopped
    pub fn mark_stopped(&mut self) {
        self.state = FacetState::Stopped;
    }

    /// Track a handle asserted through this facet
    pub fn track_handle(&mut self, handle: Handle) {
        self.handles.insert(handle);
    }

    /// Remove a tracked handle
    pub fn untrack_handle(&mut self, handle: &Handle) {
        self.handles.remove(handle);
    }

    /// Get all tracked handles
    pub fn handles(&self) -> &HashSet<Handle> {
        &self.handles
    }

    /// Track a subscription made through this facet
    pub fn track_subscription(&mut self, id: SubscriptionId) {
        self.subscriptions.insert(id);
    }

    /// Remove a tracked subscription
    pub fn untrack_subscription(&mut self, id: &SubscriptionId) {
        self.subscriptions.remove(id);
    }

    /// Get all tracked subscriptions
    pub fn subscriptions(&self) -> &HashSet<SubscriptionId> {
        &self.subscriptions
    }

    /// Take all handles (for cleanup during stop)
    pub fn take_handles(&mut self) -> HashSet<Handle> {
        std::mem::take(&mut self.handles)
    }

    /// Take all subscriptions (for cleanup during stop)
    pub fn take_subscriptions(&mut self) -> HashSet<SubscriptionId> {
        std::mem::take(&mut self.subscriptions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ActorId;

    #[test]
    fn test_root_facet() {
        let actor = ActorId::new("test");
        let facet = Facet::root(FacetId::root(actor));

        assert!(facet.is_root());
        assert!(facet.is_active());
        assert!(facet.parent().is_none());
    }

    #[test]
    fn test_child_facet() {
        let actor = ActorId::new("test");
        let root_id = FacetId::root(actor.clone());
        let child_id = FacetId::new(actor);

        let child = Facet::new(child_id, Some(root_id.clone()));

        assert!(!child.is_root());
        assert_eq!(child.parent(), Some(&root_id));
    }

    #[test]
    fn test_facet_state_transitions() {
        let actor = ActorId::new("test");
        let mut facet = Facet::root(FacetId::root(actor));

        assert!(facet.is_active());
        assert!(!facet.is_stopped());

        facet.mark_stopping();
        assert_eq!(facet.state(), FacetState::Stopping);

        facet.mark_stopped();
        assert!(facet.is_stopped());
    }

    #[test]
    fn test_handle_tracking() {
        let actor = ActorId::new("test");
        let mut facet = Facet::root(FacetId::root(actor));

        let h1 = Handle::new();
        let h2 = Handle::new();

        facet.track_handle(h1.clone());
        facet.track_handle(h2.clone());

        assert!(facet.handles().contains(&h1));
        assert!(facet.handles().contains(&h2));

        facet.untrack_handle(&h1);
        assert!(!facet.handles().contains(&h1));
        assert!(facet.handles().contains(&h2));
    }

    #[test]
    fn test_children_management() {
        let actor = ActorId::new("test");
        let mut root = Facet::root(FacetId::root(actor.clone()));

        let child1 = FacetId::new(actor.clone());
        let child2 = FacetId::new(actor);

        root.add_child(child1.clone());
        root.add_child(child2.clone());

        assert_eq!(root.children().len(), 2);

        root.remove_child(&child1);
        assert_eq!(root.children().len(), 1);
        assert!(!root.children().contains(&child1));
        assert!(root.children().contains(&child2));
    }
}
