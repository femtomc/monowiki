//! Entity trait for implementing actor behaviors
//!
//! Entities are the building blocks of actors. Each entity can respond to
//! messages, assertions, and retractions in the dataspace.

use preserves::IOValue;
use std::any::Any;

/// Trait for implementing entity behaviors in the syndicated actor model
///
/// Entities are registered on actor facets and can:
/// - Receive messages sent directly to them
/// - Be notified when matching assertions appear (on_assert)
/// - Be notified when matching assertions are retracted (on_retract)
/// - Perform cleanup when stopped
pub trait Entity: Any {
    /// Handle a message sent to this entity
    fn on_message(&mut self, message: &IOValue) {
        let _ = message;
        // Default: ignore messages
    }

    /// Handle a new assertion matching this entity's interest
    fn on_assert(&mut self, value: &IOValue) {
        let _ = value;
        // Default: ignore assertions
    }

    /// Handle retraction of a previously matching assertion
    fn on_retract(&mut self, value: &IOValue) {
        let _ = value;
        // Default: ignore retractions
    }

    /// Called when the entity is stopped (facet shutdown)
    fn on_stop(&mut self) {
        // Default: no cleanup needed
    }

    /// Snapshot entity state for persistence (optional)
    fn snapshot(&self) -> Option<IOValue> {
        None
    }

    /// Restore entity from snapshot (optional)
    fn restore(&mut self, _state: &IOValue) -> bool {
        false
    }

    /// Get the entity type name for registration
    fn entity_type(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// A simple callback entity that invokes a closure on assertions
pub struct CallbackEntity<F>
where
    F: FnMut(&IOValue) + Send + Sync,
{
    on_assert: F,
}

impl<F> CallbackEntity<F>
where
    F: FnMut(&IOValue) + Send + Sync,
{
    pub fn new(on_assert: F) -> Self {
        Self { on_assert }
    }
}

impl<F> Entity for CallbackEntity<F>
where
    F: FnMut(&IOValue) + Send + Sync + 'static,
{
    fn on_assert(&mut self, value: &IOValue) {
        (self.on_assert)(value);
    }

    fn entity_type(&self) -> &'static str {
        "CallbackEntity"
    }
}

/// A logging entity that traces assertions and retractions
pub struct LoggingEntity {
    prefix: String,
}

impl LoggingEntity {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

impl Entity for LoggingEntity {
    fn on_message(&mut self, message: &IOValue) {
        tracing::info!("{} message: {:?}", self.prefix, message);
    }

    fn on_assert(&mut self, value: &IOValue) {
        tracing::info!("{} assert: {:?}", self.prefix, value);
    }

    fn on_retract(&mut self, value: &IOValue) {
        tracing::info!("{} retract: {:?}", self.prefix, value);
    }

    fn on_stop(&mut self) {
        tracing::info!("{} stopped", self.prefix);
    }

    fn entity_type(&self) -> &'static str {
        "LoggingEntity"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_callback_entity() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let mut entity = CallbackEntity::new(move |value: &IOValue| {
            received_clone.lock().unwrap().push(value.clone());
        });

        let value = IOValue::new("test".to_string());
        entity.on_assert(&value);

        assert_eq!(received.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_default_entity_methods() {
        struct TestEntity;

        impl Entity for TestEntity {}

        let mut entity = TestEntity;
        let value = IOValue::new(42i64);

        // These should not panic (default implementations do nothing)
        entity.on_message(&value);
        entity.on_assert(&value);
        entity.on_retract(&value);
        entity.on_stop();

        assert!(entity.snapshot().is_none());
        assert!(!entity.restore(&value));
    }
}
