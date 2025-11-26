//! Entity trait for implementing actor behaviors
//!
//! Entities are the building blocks of actor behavior. Each entity
//! can respond to assertion events and messages.

use crate::dataspace::{CapabilityError, Dataspace, DataspaceRef};
use crate::types::{FacetId, Handle};
use preserves::IOValue;
use std::any::Any;
use std::collections::HashMap;

/// Context passed to entity callbacks
///
/// Provides controlled access to dataspace capabilities and facet operations.
/// Entities can only interact with dataspaces through the capabilities
/// they've been granted.
pub struct EntityContext<'a, D: Dataspace> {
    /// The facet this entity belongs to
    pub facet_id: FacetId,
    /// Available dataspace capabilities
    capabilities: &'a mut HashMap<String, DataspaceRef<D>>,
}

impl<'a, D: Dataspace> EntityContext<'a, D> {
    /// Create a new entity context
    pub fn new(facet_id: FacetId, capabilities: &'a mut HashMap<String, DataspaceRef<D>>) -> Self {
        Self {
            facet_id,
            capabilities,
        }
    }

    /// Get a capability by dataspace name
    pub fn capability(&self, name: &str) -> Option<&DataspaceRef<D>> {
        self.capabilities.get(name)
    }

    /// Get a mutable capability by dataspace name
    pub fn capability_mut(&mut self, name: &str) -> Option<&mut DataspaceRef<D>> {
        self.capabilities.get_mut(name)
    }

    /// Assert a value into a dataspace
    pub fn assert(&mut self, dataspace: &str, value: IOValue) -> Result<Handle, EntityError> {
        let cap = self
            .capabilities
            .get_mut(dataspace)
            .ok_or_else(|| EntityError::NoCapability(dataspace.to_string()))?;

        cap.assert(value).map_err(EntityError::Capability)
    }

    /// Retract an assertion from a dataspace
    pub fn retract(
        &mut self,
        dataspace: &str,
        handle: &Handle,
    ) -> Result<Option<IOValue>, EntityError> {
        let cap = self
            .capabilities
            .get_mut(dataspace)
            .ok_or_else(|| EntityError::NoCapability(dataspace.to_string()))?;

        cap.retract(handle).map_err(EntityError::Capability)
    }

    /// Query a dataspace
    pub fn query(
        &self,
        dataspace: &str,
        pattern: &crate::pattern::Pattern,
    ) -> Result<Vec<IOValue>, EntityError> {
        let cap = self
            .capabilities
            .get(dataspace)
            .ok_or_else(|| EntityError::NoCapability(dataspace.to_string()))?;

        cap.query(pattern).map_err(EntityError::Capability)
    }

    /// List available dataspace names
    pub fn dataspace_names(&self) -> impl Iterator<Item = &String> {
        self.capabilities.keys()
    }
}

/// Errors that can occur during entity operations
#[derive(Debug, thiserror::Error)]
pub enum EntityError {
    /// No capability for the requested dataspace
    #[error("no capability for dataspace: {0}")]
    NoCapability(String),

    /// Capability check failed
    #[error("capability error: {0}")]
    Capability(#[from] CapabilityError),

    /// Custom error from entity implementation
    #[error("entity error: {0}")]
    Custom(String),
}

/// Trait for implementing actor behaviors
///
/// Entities receive callbacks when:
/// - Assertions matching their subscriptions are added/removed
/// - Messages are sent directly to them
/// - Their facet is stopping
///
/// # Example
///
/// ```ignore
/// use sammy::actor::{Entity, EntityContext};
/// use sammy::dataspace::Dataspace;
/// use preserves::IOValue;
///
/// struct MyEntity {
///     count: u32,
/// }
///
/// impl<D: Dataspace> Entity<D> for MyEntity {
///     fn on_assert(&mut self, ctx: &mut EntityContext<D>, value: &IOValue) {
///         self.count += 1;
///         println!("Received assertion #{}: {:?}", self.count, value);
///     }
/// }
/// ```
pub trait Entity<D: Dataspace>: Send + Sync + Any {
    /// Called when an assertion matching a subscription is added
    ///
    /// The value has already been confirmed to match the subscription pattern.
    fn on_assert(&mut self, ctx: &mut EntityContext<D>, value: &IOValue) {
        let _ = (ctx, value);
    }

    /// Called when an assertion matching a subscription is removed
    fn on_retract(&mut self, ctx: &mut EntityContext<D>, value: &IOValue) {
        let _ = (ctx, value);
    }

    /// Called when a message is sent directly to this entity
    fn on_message(&mut self, ctx: &mut EntityContext<D>, message: &IOValue) {
        let _ = (ctx, message);
    }

    /// Called when the entity's facet is stopping
    ///
    /// This is the entity's chance to clean up resources, retract assertions,
    /// etc. After this returns, the entity will be dropped.
    fn on_stop(&mut self, ctx: &mut EntityContext<D>) {
        let _ = ctx;
    }

    /// Snapshot entity state for persistence (optional)
    ///
    /// Return None if the entity doesn't need to be persisted.
    fn snapshot(&self) -> Option<IOValue> {
        None
    }

    /// Restore entity from a snapshot (optional)
    ///
    /// Returns true if restoration was successful.
    fn restore(&mut self, _state: &IOValue) -> bool {
        false
    }

    /// Get a type name for debugging/logging
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// A no-op entity that does nothing
///
/// Useful for testing or as a placeholder.
pub struct NoopEntity;

impl<D: Dataspace> Entity<D> for NoopEntity {
    fn type_name(&self) -> &'static str {
        "NoopEntity"
    }
}

/// An entity that logs all events
pub struct LoggingEntity {
    prefix: String,
}

impl LoggingEntity {
    /// Create a new logging entity with the given prefix
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

impl<D: Dataspace> Entity<D> for LoggingEntity {
    fn on_assert(&mut self, _ctx: &mut EntityContext<D>, value: &IOValue) {
        tracing::info!("{} assert: {:?}", self.prefix, value);
    }

    fn on_retract(&mut self, _ctx: &mut EntityContext<D>, value: &IOValue) {
        tracing::info!("{} retract: {:?}", self.prefix, value);
    }

    fn on_message(&mut self, _ctx: &mut EntityContext<D>, message: &IOValue) {
        tracing::info!("{} message: {:?}", self.prefix, message);
    }

    fn on_stop(&mut self, _ctx: &mut EntityContext<D>) {
        tracing::info!("{} stopping", self.prefix);
    }

    fn type_name(&self) -> &'static str {
        "LoggingEntity"
    }
}

/// An entity that collects assertions for testing
pub struct CollectingEntity {
    /// Collected assertion values
    pub assertions: Vec<IOValue>,
    /// Collected retraction values
    pub retractions: Vec<IOValue>,
    /// Collected messages
    pub messages: Vec<IOValue>,
    /// Whether on_stop was called
    pub stopped: bool,
}

impl Default for CollectingEntity {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectingEntity {
    /// Create a new collecting entity
    pub fn new() -> Self {
        Self {
            assertions: Vec::new(),
            retractions: Vec::new(),
            messages: Vec::new(),
            stopped: false,
        }
    }

    /// Clear all collected data
    pub fn clear(&mut self) {
        self.assertions.clear();
        self.retractions.clear();
        self.messages.clear();
        self.stopped = false;
    }
}

impl<D: Dataspace> Entity<D> for CollectingEntity {
    fn on_assert(&mut self, _ctx: &mut EntityContext<D>, value: &IOValue) {
        self.assertions.push(value.clone());
    }

    fn on_retract(&mut self, _ctx: &mut EntityContext<D>, value: &IOValue) {
        self.retractions.push(value.clone());
    }

    fn on_message(&mut self, _ctx: &mut EntityContext<D>, message: &IOValue) {
        self.messages.push(message.clone());
    }

    fn on_stop(&mut self, _ctx: &mut EntityContext<D>) {
        self.stopped = true;
    }

    fn type_name(&self) -> &'static str {
        "CollectingEntity"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataspace::LocalDataspace;

    type TestDataspace = LocalDataspace;

    #[test]
    fn test_collecting_entity() {
        let mut entity = CollectingEntity::new();
        let mut caps = HashMap::new();
        let facet_id = FacetId::root(crate::types::ActorId::new("test"));
        let mut ctx: EntityContext<TestDataspace> = EntityContext::new(facet_id, &mut caps);

        let v1 = IOValue::new("test1".to_string());
        let v2 = IOValue::new("test2".to_string());

        entity.on_assert(&mut ctx, &v1);
        entity.on_message(&mut ctx, &v2);
        entity.on_stop(&mut ctx);

        assert_eq!(entity.assertions.len(), 1);
        assert_eq!(entity.messages.len(), 1);
        assert!(entity.stopped);
    }

    #[test]
    fn test_entity_context_no_capability() {
        let mut caps: HashMap<String, DataspaceRef<TestDataspace>> = HashMap::new();
        let facet_id = FacetId::root(crate::types::ActorId::new("test"));
        let mut ctx = EntityContext::new(facet_id, &mut caps);

        let result = ctx.assert("nonexistent", IOValue::new("test".to_string()));
        assert!(matches!(result, Err(EntityError::NoCapability(_))));
    }
}
