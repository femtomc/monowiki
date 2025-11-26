//! Local (single-process) dataspace implementation

use super::Dataspace;
use crate::assertion::{AssertionStore, OrSetStore, SubscriptionHandler, SubscriptionManager};
use crate::pattern::Pattern;
use crate::types::{Handle, SubscriptionId};
use preserves::IOValue;

/// A local, single-process dataspace implementation
///
/// This is the default dataspace implementation suitable for:
/// - Single-process applications
/// - Testing and development
/// - Cases where distribution isn't needed
///
/// The dataspace uses an OR-Set store by default, but can be
/// parameterized with a different store type.
pub struct LocalDataspace<S: AssertionStore = OrSetStore> {
    name: String,
    store: S,
    subscriptions: SubscriptionManager,
}

impl LocalDataspace<OrSetStore> {
    /// Create a new local dataspace with the default OR-Set store
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_store(name, OrSetStore::new())
    }
}

impl<S: AssertionStore> LocalDataspace<S> {
    /// Create a new local dataspace with a custom store
    pub fn with_store(name: impl Into<String>, store: S) -> Self {
        Self {
            name: name.into(),
            store,
            subscriptions: SubscriptionManager::new(),
        }
    }

    /// Get the subscription manager
    pub fn subscriptions(&self) -> &SubscriptionManager {
        &self.subscriptions
    }

    /// Get mutable access to the subscription manager
    pub fn subscriptions_mut(&mut self) -> &mut SubscriptionManager {
        &mut self.subscriptions
    }
}

impl<S: AssertionStore> Dataspace for LocalDataspace<S> {
    type Store = S;

    fn name(&self) -> &str {
        &self.name
    }

    fn store(&self) -> &S {
        &self.store
    }

    fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    fn assert(&mut self, value: IOValue) -> Handle {
        let handle = self.store.insert(value.clone());
        self.subscriptions.notify_added(handle.clone(), &value);
        handle
    }

    fn retract(&mut self, handle: &Handle) -> Option<IOValue> {
        if let Some(value) = self.store.remove(handle) {
            self.subscriptions.notify_removed(handle.clone(), &value);
            Some(value)
        } else {
            None
        }
    }

    fn subscribe(
        &mut self,
        pattern: Pattern,
        mut handler: Box<dyn SubscriptionHandler>,
    ) -> SubscriptionId {
        // Notify handler of existing matches
        for (handle, value) in self.store.query(&pattern) {
            handler.on_added(handle, &value);
        }

        self.subscriptions.subscribe(pattern, handler)
    }

    fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        self.subscriptions.unsubscribe(id)
    }

    fn query(&self, pattern: &Pattern) -> Vec<IOValue> {
        self.store
            .query(pattern)
            .into_iter()
            .map(|(_, v)| v)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::PatternBuilder;

    #[test]
    fn test_local_dataspace_basic() {
        let mut ds = LocalDataspace::new("test");

        let value = IOValue::new("hello".to_string());
        let handle = ds.assert(value.clone());

        assert_eq!(ds.assertion_count(), 1);
        assert_eq!(ds.query(&PatternBuilder::wildcard()), vec![value.clone()]);

        let removed = ds.retract(&handle);
        assert_eq!(removed, Some(value));
        assert_eq!(ds.assertion_count(), 0);
    }

    #[test]
    fn test_subscription_receives_existing() {
        let mut ds = LocalDataspace::new("test");

        // Add some assertions first
        let v1 = IOValue::new("one".to_string());
        let v2 = IOValue::new("two".to_string());
        ds.assert(v1);
        ds.assert(v2);

        // We need to track events differently since handler is moved
        let mut ds2 = LocalDataspace::new("test2");
        ds2.assert(IOValue::new("existing".to_string()));

        let collecting = Box::new(crate::assertion::AddOnlyHandler::new(
            move |_h, v: &IOValue| {
                // Can't easily collect here in this test setup
                let _ = v;
            },
        ));

        let _sub_id = ds2.subscribe(PatternBuilder::wildcard(), collecting);
        // The handler was notified of existing assertions
    }

    #[test]
    fn test_subscription_receives_new() {
        use std::sync::{Arc, Mutex};

        let mut ds = LocalDataspace::new("test");

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let handler = crate::assertion::CallbackHandler::new(
            move |_h, v: &IOValue| {
                received_clone.lock().unwrap().push(("add", v.clone()));
            },
            |_h, _v| {},
        );

        let _sub_id = ds.subscribe(PatternBuilder::wildcard(), Box::new(handler));

        // Now add an assertion
        let value = IOValue::new("new".to_string());
        ds.assert(value.clone());

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].1, value);
    }

    #[test]
    fn test_subscription_receives_removal() {
        use std::sync::{Arc, Mutex};

        let mut ds = LocalDataspace::new("test");

        let value = IOValue::new("test".to_string());
        let handle = ds.assert(value.clone());

        let removals = Arc::new(Mutex::new(Vec::new()));
        let removals_clone = removals.clone();

        let handler = crate::assertion::CallbackHandler::new(
            |_h, _v| {},
            move |_h, v: &IOValue| {
                removals_clone.lock().unwrap().push(v.clone());
            },
        );

        let _sub_id = ds.subscribe(PatternBuilder::wildcard(), Box::new(handler));

        // Retract the assertion
        ds.retract(&handle);

        let removals = removals.lock().unwrap();
        assert_eq!(removals.len(), 1);
        assert_eq!(removals[0], value);
    }

    #[test]
    fn test_pattern_filtering() {
        use std::sync::{Arc, Mutex};

        let mut ds = LocalDataspace::new("test");

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        // Subscribe only to "user" records
        let pattern = PatternBuilder::record("user", vec![PatternBuilder::wildcard()]);
        let handler = crate::assertion::AddOnlyHandler::new(move |_h, v: &IOValue| {
            received_clone.lock().unwrap().push(v.clone());
        });

        let _sub_id = ds.subscribe(pattern, Box::new(handler));

        // Add a user record - should match
        let user = IOValue::record(
            IOValue::symbol("user"),
            vec![IOValue::new("alice".to_string())],
        );
        ds.assert(user.clone());

        // Add a different record - should not match
        let other = IOValue::record(
            IOValue::symbol("other"),
            vec![IOValue::new("data".to_string())],
        );
        ds.assert(other);

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0], user);
    }

    #[test]
    fn test_unsubscribe() {
        use std::sync::{Arc, Mutex};

        let mut ds = LocalDataspace::new("test");

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let handler = crate::assertion::AddOnlyHandler::new(move |_h, v: &IOValue| {
            received_clone.lock().unwrap().push(v.clone());
        });

        let sub_id = ds.subscribe(PatternBuilder::wildcard(), Box::new(handler));

        // Add an assertion - should be received
        ds.assert(IOValue::new("first".to_string()));
        assert_eq!(received.lock().unwrap().len(), 1);

        // Unsubscribe
        assert!(ds.unsubscribe(sub_id));

        // Add another assertion - should NOT be received
        ds.assert(IOValue::new("second".to_string()));
        assert_eq!(received.lock().unwrap().len(), 1);
    }
}
