//! Runtime coordinator for the syndicated actor model
//!
//! The runtime manages:
//! - Dataspaces (creation, lookup)
//! - Actors (spawning, lifecycle)
//! - Capabilities (granting, attenuation)
//! - Turn scheduling (for deterministic execution)

use crate::actor::Actor;
use crate::assertion::OrSetStore;
use crate::dataspace::{Dataspace, DataspaceRef, LocalDataspace, Permissions};
use crate::types::{ActorId, TurnId};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration trait for customizing the runtime
///
/// Implementations can provide custom dataspace and store types
/// to suit different needs (persistence, distribution, etc.)
pub trait RuntimeConfig: Send + Sync {
    /// The dataspace type to use
    type Dataspace: Dataspace;

    /// Create a new dataspace with the given name
    fn create_dataspace(&self, name: &str) -> Self::Dataspace;
}

/// Default runtime configuration using local dataspaces with OR-Set stores
#[derive(Debug, Clone, Default)]
pub struct DefaultConfig;

impl RuntimeConfig for DefaultConfig {
    type Dataspace = LocalDataspace<OrSetStore>;

    fn create_dataspace(&self, name: &str) -> Self::Dataspace {
        LocalDataspace::new(name)
    }
}

/// The main runtime coordinator
///
/// Manages the lifecycle of dataspaces and actors, and coordinates
/// capability-based access control.
///
/// # Type Parameters
///
/// * `C` - The runtime configuration type
///
/// # Example
///
/// ```
/// use sammy::runtime::{Runtime, DefaultConfig};
/// use sammy::dataspace::Permissions;
///
/// let mut runtime = Runtime::new(DefaultConfig);
///
/// // Create a dataspace
/// let ds = runtime.dataspace("my-space");
///
/// // Spawn an actor with a capability
/// let actor_id = runtime.spawn_actor("my-actor");
/// runtime.grant_capability(&actor_id, "my-space", Permissions::full());
/// ```
pub struct Runtime<C: RuntimeConfig> {
    /// Configuration
    config: C,
    /// Active dataspaces
    dataspaces: HashMap<String, Arc<RwLock<C::Dataspace>>>,
    /// Active actors
    actors: HashMap<ActorId, Actor<C::Dataspace>>,
    /// Current turn ID (for deterministic ordering)
    current_turn: TurnId,
}

impl Runtime<DefaultConfig> {
    /// Create a new runtime with default configuration
    pub fn with_defaults() -> Self {
        Self::new(DefaultConfig)
    }
}

impl<C: RuntimeConfig> Runtime<C> {
    /// Create a new runtime with the given configuration
    pub fn new(config: C) -> Self {
        Self {
            config,
            dataspaces: HashMap::new(),
            actors: HashMap::new(),
            current_turn: TurnId::genesis(),
        }
    }

    /// Get or create a dataspace by name
    ///
    /// If the dataspace doesn't exist, it will be created using the
    /// runtime's configuration.
    pub fn dataspace(&mut self, name: &str) -> Arc<RwLock<C::Dataspace>> {
        self.dataspaces
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(self.config.create_dataspace(name))))
            .clone()
    }

    /// Check if a dataspace exists
    pub fn has_dataspace(&self, name: &str) -> bool {
        self.dataspaces.contains_key(name)
    }

    /// List all dataspace names
    pub fn dataspace_names(&self) -> impl Iterator<Item = &String> {
        self.dataspaces.keys()
    }

    /// Spawn a new actor
    ///
    /// Returns the actor's ID. The actor starts with no capabilities;
    /// use `grant_capability` to give it access to dataspaces.
    pub fn spawn_actor(&mut self, name: impl Into<String>) -> ActorId {
        let id = ActorId::new(name);
        let actor = Actor::new(id.clone());
        self.actors.insert(id.clone(), actor);
        id
    }

    /// Get an actor by ID
    pub fn actor(&self, id: &ActorId) -> Option<&Actor<C::Dataspace>> {
        self.actors.get(id)
    }

    /// Get a mutable actor by ID
    pub fn actor_mut(&mut self, id: &ActorId) -> Option<&mut Actor<C::Dataspace>> {
        self.actors.get_mut(id)
    }

    /// Stop an actor
    ///
    /// This stops all the actor's facets and removes it from the runtime.
    pub fn stop_actor(&mut self, id: &ActorId) -> bool {
        if let Some(mut actor) = self.actors.remove(id) {
            actor.stop();
            true
        } else {
            false
        }
    }

    /// List all actor IDs
    pub fn actor_ids(&self) -> impl Iterator<Item = &ActorId> {
        self.actors.keys()
    }

    /// Grant a capability to an actor
    ///
    /// The actor will be able to interact with the named dataspace
    /// according to the given permissions.
    pub fn grant_capability(
        &mut self,
        actor_id: &ActorId,
        dataspace_name: &str,
        permissions: Permissions,
    ) -> bool {
        // Ensure dataspace exists
        let ds = self.dataspace(dataspace_name);

        if let Some(actor) = self.actors.get_mut(actor_id) {
            let cap = DataspaceRef::new(dataspace_name, ds, permissions);
            actor.grant_capability(dataspace_name.to_string(), cap);
            true
        } else {
            false
        }
    }

    /// Create a standalone capability (not tied to an actor)
    ///
    /// Useful for creating capabilities to pass to entities or
    /// for testing.
    pub fn create_capability(
        &mut self,
        dataspace_name: &str,
        permissions: Permissions,
    ) -> DataspaceRef<C::Dataspace> {
        let ds = self.dataspace(dataspace_name);
        DataspaceRef::new(dataspace_name, ds, permissions)
    }

    /// Get the current turn ID
    pub fn current_turn(&self) -> &TurnId {
        &self.current_turn
    }

    /// Advance to the next turn
    ///
    /// In a full implementation, this would process pending events
    /// and execute entity callbacks.
    pub fn advance_turn(&mut self) -> TurnId {
        self.current_turn = TurnId::new();
        self.current_turn.clone()
    }

    /// Get statistics about the runtime
    pub fn stats(&self) -> RuntimeStats {
        let total_assertions: usize = self
            .dataspaces
            .values()
            .map(|ds| ds.read().assertion_count())
            .sum();

        RuntimeStats {
            dataspace_count: self.dataspaces.len(),
            actor_count: self.actors.len(),
            total_assertions,
        }
    }
}

/// Statistics about the runtime
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    /// Number of active dataspaces
    pub dataspace_count: usize,
    /// Number of active actors
    pub actor_count: usize,
    /// Total assertions across all dataspaces
    pub total_assertions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::PatternBuilder;
    use preserves::IOValue;

    #[test]
    fn test_runtime_creation() {
        let runtime = Runtime::with_defaults();
        assert_eq!(runtime.stats().dataspace_count, 0);
        assert_eq!(runtime.stats().actor_count, 0);
    }

    #[test]
    fn test_dataspace_creation() {
        let mut runtime = Runtime::with_defaults();

        let _ds1 = runtime.dataspace("test");
        assert!(runtime.has_dataspace("test"));
        assert_eq!(runtime.stats().dataspace_count, 1);

        // Getting the same dataspace again shouldn't create a new one
        let _ds2 = runtime.dataspace("test");
        assert_eq!(runtime.stats().dataspace_count, 1);
    }

    #[test]
    fn test_actor_lifecycle() {
        let mut runtime = Runtime::with_defaults();

        let actor_id = runtime.spawn_actor("test-actor");
        assert!(runtime.actor(&actor_id).is_some());
        assert_eq!(runtime.stats().actor_count, 1);

        assert!(runtime.stop_actor(&actor_id));
        assert!(runtime.actor(&actor_id).is_none());
        assert_eq!(runtime.stats().actor_count, 0);
    }

    #[test]
    fn test_grant_capability() {
        let mut runtime = Runtime::with_defaults();

        let actor_id = runtime.spawn_actor("test-actor");
        runtime.grant_capability(&actor_id, "my-ds", Permissions::full());

        let actor = runtime.actor(&actor_id).unwrap();
        assert!(actor.capability("my-ds").is_some());
    }

    #[test]
    fn test_capability_usage() {
        let mut runtime = Runtime::with_defaults();

        let actor_id = runtime.spawn_actor("test-actor");
        runtime.grant_capability(&actor_id, "my-ds", Permissions::full());

        // Use the capability to assert something
        {
            let actor = runtime.actor_mut(&actor_id).unwrap();
            let cap = actor.capability_mut("my-ds").unwrap();
            let value = IOValue::new("hello".to_string());
            cap.assert(value).unwrap();
        }

        // Verify the assertion is in the dataspace
        let ds = runtime.dataspace("my-ds");
        let results = ds.read().query(&PatternBuilder::wildcard());
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_turn_advancement() {
        let mut runtime = Runtime::with_defaults();

        let t1 = runtime.current_turn().clone();
        let t2 = runtime.advance_turn();

        assert!(t2 > t1);
    }

    #[test]
    fn test_custom_config() {
        // Example of a custom config (same as default for this test)
        #[derive(Clone)]
        struct MyConfig;

        impl RuntimeConfig for MyConfig {
            type Dataspace = LocalDataspace<OrSetStore>;

            fn create_dataspace(&self, name: &str) -> Self::Dataspace {
                // Could customize dataspace creation here
                LocalDataspace::new(format!("custom-{}", name))
            }
        }

        let mut runtime = Runtime::new(MyConfig);
        let ds = runtime.dataspace("test");

        // The custom config prefixes the name
        assert_eq!(ds.read().name(), "custom-test");
    }
}
