//! Dataspace client for actor communication
//!
//! This module provides a stub implementation of the dataspace client for
//! live cells to communicate with the syndicated actor runtime.
//!
//! In the full implementation, this will integrate with the syndicate runtime
//! to enable publish/subscribe patterns and actor coordination.

use crate::abi::{RuntimeError, RuntimeResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Assertion identifier
pub type AssertionId = u64;

/// Subscription identifier
pub type SubscriptionId = u64;

/// Callback identifier for dataspace subscriptions
pub type CallbackId = u64;

/// An assertion in the dataspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    pub id: AssertionId,
    pub pattern: String,
    pub value: Vec<u8>,
}

/// A subscription to a pattern
#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: SubscriptionId,
    pub pattern: String,
    pub callback: CallbackId,
}

/// Stub dataspace client for live cells
///
/// This is a minimal implementation that stores assertions locally.
/// In the full implementation, this will communicate with the syndicated
/// actor runtime to publish and subscribe to shared dataspaces.
#[derive(Debug)]
pub struct DataspaceClient {
    assertions: HashMap<AssertionId, Assertion>,
    subscriptions: HashMap<SubscriptionId, Subscription>,
    next_assertion_id: AtomicU64,
    next_subscription_id: AtomicU64,
    // Pattern -> AssertionIds for efficient subscription matching
    pattern_index: HashMap<String, Vec<AssertionId>>,
}

impl DataspaceClient {
    pub fn new() -> Self {
        Self {
            assertions: HashMap::new(),
            subscriptions: HashMap::new(),
            next_assertion_id: AtomicU64::new(1),
            next_subscription_id: AtomicU64::new(1),
            pattern_index: HashMap::new(),
        }
    }

    /// Publish an assertion to the dataspace
    ///
    /// Returns the assertion ID that can be used to retract it later.
    pub fn publish(&mut self, pattern: String, value: Vec<u8>) -> AssertionId {
        let id = self.next_assertion_id.fetch_add(1, Ordering::SeqCst);

        let assertion = Assertion {
            id,
            pattern: pattern.clone(),
            value,
        };

        self.assertions.insert(id, assertion);

        // Update pattern index
        self.pattern_index
            .entry(pattern)
            .or_insert_with(Vec::new)
            .push(id);

        id
    }

    /// Retract an assertion from the dataspace
    pub fn retract(&mut self, assertion_id: AssertionId) -> RuntimeResult<()> {
        if let Some(assertion) = self.assertions.remove(&assertion_id) {
            // Remove from pattern index
            if let Some(ids) = self.pattern_index.get_mut(&assertion.pattern) {
                ids.retain(|&id| id != assertion_id);
                if ids.is_empty() {
                    self.pattern_index.remove(&assertion.pattern);
                }
            }
            Ok(())
        } else {
            Err(RuntimeError::DataspaceError(format!(
                "Assertion {} not found",
                assertion_id
            )))
        }
    }

    /// Subscribe to a pattern in the dataspace
    ///
    /// Returns the subscription ID that can be used to unsubscribe later.
    pub fn subscribe(&mut self, pattern: String, callback_id: CallbackId) -> SubscriptionId {
        let id = self.next_subscription_id.fetch_add(1, Ordering::SeqCst);

        let subscription = Subscription {
            id,
            pattern,
            callback: callback_id,
        };

        self.subscriptions.insert(id, subscription);

        id
    }

    /// Unsubscribe from a pattern
    pub fn unsubscribe(&mut self, subscription_id: SubscriptionId) -> RuntimeResult<()> {
        if self.subscriptions.remove(&subscription_id).is_some() {
            Ok(())
        } else {
            Err(RuntimeError::DataspaceError(format!(
                "Subscription {} not found",
                subscription_id
            )))
        }
    }

    /// Query assertions matching a pattern
    ///
    /// This is a simple implementation that does exact pattern matching.
    /// In the full implementation, this would support pattern matching with
    /// wildcards and structural patterns.
    pub fn query(&self, pattern: &str) -> Vec<&Assertion> {
        self.pattern_index
            .get(pattern)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.assertions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get subscriptions for a pattern
    ///
    /// Returns callbacks that should be notified when an assertion
    /// matching the pattern is published.
    pub fn subscribers(&self, pattern: &str) -> Vec<CallbackId> {
        self.subscriptions
            .values()
            .filter(|sub| self.pattern_matches(&sub.pattern, pattern))
            .map(|sub| sub.callback)
            .collect()
    }

    /// Simple pattern matching (exact match for now)
    ///
    /// In the full implementation, this would support wildcards and
    /// structural pattern matching.
    fn pattern_matches(&self, subscription_pattern: &str, assertion_pattern: &str) -> bool {
        subscription_pattern == assertion_pattern
    }

    /// Get all assertions
    pub fn assertions(&self) -> impl Iterator<Item = &Assertion> {
        self.assertions.values()
    }

    /// Get all subscriptions
    pub fn subscriptions(&self) -> impl Iterator<Item = &Subscription> {
        self.subscriptions.values()
    }

    /// Get the number of assertions
    pub fn assertion_count(&self) -> usize {
        self.assertions.len()
    }

    /// Get the number of subscriptions
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Clear all assertions and subscriptions
    pub fn clear(&mut self) {
        self.assertions.clear();
        self.subscriptions.clear();
        self.pattern_index.clear();
    }
}

impl Default for DataspaceClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_and_query() {
        let mut client = DataspaceClient::new();

        let id1 = client.publish("user.login".to_string(), vec![1, 2, 3]);
        let id2 = client.publish("user.login".to_string(), vec![4, 5, 6]);
        let _id3 = client.publish("user.logout".to_string(), vec![7, 8, 9]);

        let results = client.query("user.login");
        assert_eq!(results.len(), 2);

        let ids: Vec<AssertionId> = results.iter().map(|a| a.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn test_retract() {
        let mut client = DataspaceClient::new();

        let id = client.publish("test.pattern".to_string(), vec![1, 2, 3]);

        let results = client.query("test.pattern");
        assert_eq!(results.len(), 1);

        client.retract(id).unwrap();

        let results = client.query("test.pattern");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_subscribe() {
        let mut client = DataspaceClient::new();

        let sub_id = client.subscribe("user.login".to_string(), 100);

        let subscribers = client.subscribers("user.login");
        assert_eq!(subscribers, vec![100]);

        client.unsubscribe(sub_id).unwrap();

        let subscribers = client.subscribers("user.login");
        assert_eq!(subscribers.len(), 0);
    }

    #[test]
    fn test_multiple_subscriptions() {
        let mut client = DataspaceClient::new();

        client.subscribe("test.pattern".to_string(), 1);
        client.subscribe("test.pattern".to_string(), 2);
        client.subscribe("test.pattern".to_string(), 3);

        let subscribers = client.subscribers("test.pattern");
        assert_eq!(subscribers.len(), 3);
        assert!(subscribers.contains(&1));
        assert!(subscribers.contains(&2));
        assert!(subscribers.contains(&3));
    }

    #[test]
    fn test_assertion_count() {
        let mut client = DataspaceClient::new();

        assert_eq!(client.assertion_count(), 0);

        let id1 = client.publish("test1".to_string(), vec![1]);
        let id2 = client.publish("test2".to_string(), vec![2]);

        assert_eq!(client.assertion_count(), 2);

        client.retract(id1).unwrap();
        assert_eq!(client.assertion_count(), 1);

        client.retract(id2).unwrap();
        assert_eq!(client.assertion_count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut client = DataspaceClient::new();

        client.publish("test".to_string(), vec![1, 2, 3]);
        client.subscribe("test".to_string(), 100);

        assert_eq!(client.assertion_count(), 1);
        assert_eq!(client.subscription_count(), 1);

        client.clear();

        assert_eq!(client.assertion_count(), 0);
        assert_eq!(client.subscription_count(), 0);
    }
}
