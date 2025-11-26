//! Subscription handling for assertion changes
//!
//! This module provides abstractions for notifying subscribers
//! when assertions matching their patterns are added or removed.

use crate::pattern::Pattern;
use crate::types::{Handle, SubscriptionId};
use preserves::IOValue;
use std::collections::HashMap;

/// Event emitted when an assertion changes
#[derive(Debug, Clone)]
pub enum AssertionEvent {
    /// A new assertion was added
    Added {
        /// Handle for the assertion
        handle: Handle,
        /// The assertion value
        value: IOValue,
    },
    /// An assertion was removed
    Removed {
        /// Handle of the removed assertion
        handle: Handle,
        /// The value that was removed
        value: IOValue,
    },
}

impl AssertionEvent {
    /// Get the handle from the event
    pub fn handle(&self) -> &Handle {
        match self {
            AssertionEvent::Added { handle, .. } => handle,
            AssertionEvent::Removed { handle, .. } => handle,
        }
    }

    /// Get the value from the event
    pub fn value(&self) -> &IOValue {
        match self {
            AssertionEvent::Added { value, .. } => value,
            AssertionEvent::Removed { value, .. } => value,
        }
    }

    /// Check if this is an add event
    pub fn is_added(&self) -> bool {
        matches!(self, AssertionEvent::Added { .. })
    }

    /// Check if this is a remove event
    pub fn is_removed(&self) -> bool {
        matches!(self, AssertionEvent::Removed { .. })
    }
}

/// Trait for handling assertion events
///
/// Implementations receive notifications when assertions matching
/// their subscription pattern are added or removed.
pub trait SubscriptionHandler: Send + Sync {
    /// Called when a matching assertion is added
    fn on_added(&mut self, handle: Handle, value: &IOValue);

    /// Called when a matching assertion is removed
    fn on_removed(&mut self, handle: Handle, value: &IOValue);

    /// Convenience method to handle an event
    fn handle_event(&mut self, event: AssertionEvent) {
        match event {
            AssertionEvent::Added { handle, value } => self.on_added(handle, &value),
            AssertionEvent::Removed { handle, value } => self.on_removed(handle, &value),
        }
    }
}

/// A subscription handler that calls closures
pub struct CallbackHandler<A, R>
where
    A: FnMut(Handle, &IOValue) + Send + Sync,
    R: FnMut(Handle, &IOValue) + Send + Sync,
{
    on_add: A,
    on_remove: R,
}

impl<A, R> CallbackHandler<A, R>
where
    A: FnMut(Handle, &IOValue) + Send + Sync,
    R: FnMut(Handle, &IOValue) + Send + Sync,
{
    /// Create a new callback handler
    pub fn new(on_add: A, on_remove: R) -> Self {
        Self {
            on_add,
            on_remove,
        }
    }
}

impl<A, R> SubscriptionHandler for CallbackHandler<A, R>
where
    A: FnMut(Handle, &IOValue) + Send + Sync,
    R: FnMut(Handle, &IOValue) + Send + Sync,
{
    fn on_added(&mut self, handle: Handle, value: &IOValue) {
        (self.on_add)(handle, value);
    }

    fn on_removed(&mut self, handle: Handle, value: &IOValue) {
        (self.on_remove)(handle, value);
    }
}

/// A subscription handler that only cares about additions
pub struct AddOnlyHandler<F>
where
    F: FnMut(Handle, &IOValue) + Send + Sync,
{
    handler: F,
}

impl<F> AddOnlyHandler<F>
where
    F: FnMut(Handle, &IOValue) + Send + Sync,
{
    /// Create a new add-only handler
    pub fn new(handler: F) -> Self {
        Self { handler }
    }
}

impl<F> SubscriptionHandler for AddOnlyHandler<F>
where
    F: FnMut(Handle, &IOValue) + Send + Sync,
{
    fn on_added(&mut self, handle: Handle, value: &IOValue) {
        (self.handler)(handle, value);
    }

    fn on_removed(&mut self, _handle: Handle, _value: &IOValue) {
        // Ignore removals
    }
}

/// A subscription handler that collects events
#[derive(Debug, Default)]
pub struct CollectingHandler {
    /// Collected events
    pub events: Vec<AssertionEvent>,
}

impl CollectingHandler {
    /// Create a new collecting handler
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the collected events, leaving the handler empty
    pub fn take(&mut self) -> Vec<AssertionEvent> {
        std::mem::take(&mut self.events)
    }

    /// Clear collected events
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

impl SubscriptionHandler for CollectingHandler {
    fn on_added(&mut self, handle: Handle, value: &IOValue) {
        self.events.push(AssertionEvent::Added {
            handle,
            value: value.clone(),
        });
    }

    fn on_removed(&mut self, handle: Handle, value: &IOValue) {
        self.events.push(AssertionEvent::Removed {
            handle,
            value: value.clone(),
        });
    }
}

/// A registered subscription with its pattern and handler
struct Subscription {
    pattern: Pattern,
    handler: Box<dyn SubscriptionHandler>,
}

/// Manages subscriptions and dispatches events
///
/// The subscription manager maintains a set of pattern-based subscriptions
/// and efficiently dispatches assertion events to matching handlers.
pub struct SubscriptionManager {
    subscriptions: HashMap<SubscriptionId, Subscription>,
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionManager {
    /// Create a new subscription manager
    pub fn new() -> Self {
        Self {
            subscriptions: HashMap::new(),
        }
    }

    /// Subscribe to assertions matching a pattern
    pub fn subscribe(
        &mut self,
        pattern: Pattern,
        handler: Box<dyn SubscriptionHandler>,
    ) -> SubscriptionId {
        let id = SubscriptionId::new();
        self.subscriptions.insert(id, Subscription { pattern, handler });
        id
    }

    /// Unsubscribe
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        self.subscriptions.remove(&id).is_some()
    }

    /// Get the number of active subscriptions
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Check if there are no subscriptions
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }

    /// Notify subscribers of an assertion being added
    pub fn notify_added(&mut self, handle: Handle, value: &IOValue) {
        for sub in self.subscriptions.values_mut() {
            if sub.pattern.matches_tagged(value) {
                sub.handler.on_added(handle.clone(), value);
            }
        }
    }

    /// Notify subscribers of an assertion being removed
    pub fn notify_removed(&mut self, handle: Handle, value: &IOValue) {
        for sub in self.subscriptions.values_mut() {
            if sub.pattern.matches_tagged(value) {
                sub.handler.on_removed(handle.clone(), value);
            }
        }
    }

    /// Get patterns for all active subscriptions (for debugging)
    pub fn patterns(&self) -> impl Iterator<Item = (SubscriptionId, &Pattern)> {
        self.subscriptions.iter().map(|(id, sub)| (*id, &sub.pattern))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::PatternBuilder;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_collecting_handler() {
        let mut handler = CollectingHandler::new();

        let h1 = Handle::new();
        let h2 = Handle::new();
        let v1 = IOValue::new("test1".to_string());
        let v2 = IOValue::new("test2".to_string());

        handler.on_added(h1.clone(), &v1);
        handler.on_removed(h2.clone(), &v2);

        assert_eq!(handler.events.len(), 2);
        assert!(handler.events[0].is_added());
        assert!(handler.events[1].is_removed());
    }

    #[test]
    fn test_subscription_manager() {
        let mut manager = SubscriptionManager::new();

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let handler = AddOnlyHandler::new(move |_handle, value| {
            received_clone.lock().unwrap().push(value.clone());
        });

        let pattern = PatternBuilder::record("user", vec![PatternBuilder::wildcard()]);
        let _sub_id = manager.subscribe(pattern, Box::new(handler));

        // Add a matching assertion
        let user = IOValue::record(
            IOValue::symbol("user"),
            vec![IOValue::new("alice".to_string())],
        );
        manager.notify_added(Handle::new(), &user);

        // Add a non-matching assertion
        let other = IOValue::new("not a user".to_string());
        manager.notify_added(Handle::new(), &other);

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1);
    }

    #[test]
    fn test_unsubscribe() {
        let mut manager = SubscriptionManager::new();

        let handler = CollectingHandler::new();
        let pattern = PatternBuilder::wildcard();
        let sub_id = manager.subscribe(pattern, Box::new(handler));

        assert_eq!(manager.len(), 1);

        assert!(manager.unsubscribe(sub_id));
        assert_eq!(manager.len(), 0);

        // Double unsubscribe returns false
        assert!(!manager.unsubscribe(sub_id));
    }
}
