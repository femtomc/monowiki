//! Dataspace client for actor communication
//!
//! This module provides the dataspace client that bridges WASM live cells
//! to the sammy syndicated actor runtime. It wraps sammy's capability-based
//! access with a simpler API suitable for host function implementations.

use crate::abi::{RuntimeError, RuntimeResult};
use parking_lot::RwLock;
use preserves::IOValue;
use sammy::assertion::OrSetStore;
use sammy::dataspace::{LocalDataspace, Permissions};
use sammy::{Dataspace, Handle, Pattern, PatternBuilder, SubscriptionId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Assertion identifier (wraps sammy Handle)
pub type AssertionId = u64;

/// Callback identifier for dataspace subscriptions
pub type CallbackId = u64;

/// A thin wrapper around sammy's dataspace for live cell access
///
/// This provides a simplified API that:
/// - Uses u64 IDs instead of Handle/SubscriptionId types
/// - Converts between byte slices and IOValue
/// - Maintains callback mappings for subscription notifications
pub struct DataspaceClient {
    /// Reference to the underlying sammy dataspace
    dataspace: Arc<RwLock<LocalDataspace<OrSetStore>>>,
    /// Permissions for this client
    permissions: Permissions,
    /// Mapping from u64 assertion IDs to sammy Handles
    handles: HashMap<u64, Handle>,
    /// Mapping from u64 subscription IDs to sammy SubscriptionIds
    subscriptions: HashMap<u64, SubscriptionId>,
    /// Mapping from subscription IDs to callback IDs
    callbacks: HashMap<u64, CallbackId>,
    /// Next assertion ID to allocate
    next_id: AtomicU64,
}

impl DataspaceClient {
    /// Create a new dataspace client with full permissions
    pub fn new(dataspace: Arc<RwLock<LocalDataspace<OrSetStore>>>) -> Self {
        Self {
            dataspace,
            permissions: Permissions::full(),
            handles: HashMap::new(),
            subscriptions: HashMap::new(),
            callbacks: HashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Create a new dataspace client with specific permissions
    pub fn with_permissions(
        dataspace: Arc<RwLock<LocalDataspace<OrSetStore>>>,
        permissions: Permissions,
    ) -> Self {
        Self {
            dataspace,
            permissions,
            handles: HashMap::new(),
            subscriptions: HashMap::new(),
            callbacks: HashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    /// Allocate a new ID
    fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Publish an assertion to the dataspace
    ///
    /// The pattern string is used as a record label, and the value bytes
    /// are stored as the record's payload.
    pub fn publish(&mut self, pattern: String, value: Vec<u8>) -> RuntimeResult<u64> {
        // Check assert permission
        let iovalue = IOValue::record(
            IOValue::symbol(pattern.clone()),
            vec![IOValue::new(value)],
        );

        if let Some(ref filter) = self.permissions.assert_filter {
            if !filter.matches_tagged(&iovalue) {
                return Err(RuntimeError::CapabilityError(format!(
                    "Assertion does not match permitted pattern: {}",
                    pattern
                )));
            }
        }

        let mut ds = self.dataspace.write();
        let handle = ds.assert(iovalue);

        let id = self.alloc_id();
        self.handles.insert(id, handle);
        Ok(id)
    }

    /// Publish a raw IOValue assertion
    pub fn publish_iovalue(&mut self, value: IOValue) -> RuntimeResult<u64> {
        if let Some(ref filter) = self.permissions.assert_filter {
            if !filter.matches_tagged(&value) {
                return Err(RuntimeError::CapabilityError(
                    "Assertion does not match permitted pattern".to_string(),
                ));
            }
        }

        let mut ds = self.dataspace.write();
        let handle = ds.assert(value);

        let id = self.alloc_id();
        self.handles.insert(id, handle);
        Ok(id)
    }

    /// Retract an assertion from the dataspace
    pub fn retract(&mut self, assertion_id: u64) -> RuntimeResult<()> {
        let handle = self.handles.remove(&assertion_id).ok_or_else(|| {
            RuntimeError::DataspaceError(format!("Assertion {} not found", assertion_id))
        })?;

        // Check retract permission
        if !self.permissions.can_retract_own && !self.permissions.can_retract_any {
            return Err(RuntimeError::CapabilityError(
                "No permission to retract assertions".to_string(),
            ));
        }

        let mut ds = self.dataspace.write();
        ds.retract(&handle);
        Ok(())
    }

    /// Subscribe to a pattern in the dataspace
    ///
    /// Returns a subscription ID. When assertions matching the pattern
    /// are added or removed, the callback_id will be notified.
    pub fn subscribe(&mut self, pattern: String, callback_id: CallbackId) -> RuntimeResult<u64> {
        // Create a pattern that matches records with this label
        let pat = PatternBuilder::record(&pattern, vec![PatternBuilder::wildcard()]);

        // Check observe permission
        if let Some(ref filter) = self.permissions.observe_filter {
            // For now, just check that the pattern label matches
            // A more sophisticated check would verify pattern intersection
            let _ = filter; // TODO: proper pattern intersection check
        }

        // Create a collecting handler that we can poll later
        let handler = Box::new(NotifyHandler::new(callback_id));

        let mut ds = self.dataspace.write();
        let sub_id = ds.subscribe(pat, handler);

        let id = self.alloc_id();
        self.subscriptions.insert(id, sub_id);
        self.callbacks.insert(id, callback_id);
        Ok(id)
    }

    /// Subscribe with a raw pattern
    pub fn subscribe_pattern(
        &mut self,
        pattern: Pattern,
        callback_id: CallbackId,
    ) -> RuntimeResult<u64> {
        if let Some(ref filter) = self.permissions.observe_filter {
            let _ = filter; // TODO: pattern intersection check
        }

        let handler = Box::new(NotifyHandler::new(callback_id));

        let mut ds = self.dataspace.write();
        let sub_id = ds.subscribe(pattern, handler);

        let id = self.alloc_id();
        self.subscriptions.insert(id, sub_id);
        self.callbacks.insert(id, callback_id);
        Ok(id)
    }

    /// Unsubscribe from a pattern
    pub fn unsubscribe(&mut self, subscription_id: u64) -> RuntimeResult<()> {
        let sub_id = self.subscriptions.remove(&subscription_id).ok_or_else(|| {
            RuntimeError::DataspaceError(format!("Subscription {} not found", subscription_id))
        })?;

        self.callbacks.remove(&subscription_id);

        let mut ds = self.dataspace.write();
        ds.unsubscribe(sub_id);
        Ok(())
    }

    /// Query assertions matching a pattern string
    pub fn query(&self, pattern: &str) -> Vec<IOValue> {
        let pat = PatternBuilder::record(pattern, vec![PatternBuilder::wildcard()]);
        self.query_pattern(&pat)
    }

    /// Query assertions matching a pattern
    pub fn query_pattern(&self, pattern: &Pattern) -> Vec<IOValue> {
        if let Some(ref filter) = self.permissions.observe_filter {
            // TODO: pattern intersection check
            let _ = filter;
        }

        let ds = self.dataspace.read();
        ds.query(pattern)
    }

    /// Get the number of active assertions
    pub fn assertion_count(&self) -> usize {
        self.dataspace.read().assertion_count()
    }

    /// Get the number of active subscriptions for this client
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Clear all assertions and subscriptions made by this client
    pub fn clear(&mut self) {
        let mut ds = self.dataspace.write();

        // Retract all our assertions
        for (_, handle) in self.handles.drain() {
            ds.retract(&handle);
        }

        // Unsubscribe from all our subscriptions
        for (_, sub_id) in self.subscriptions.drain() {
            ds.unsubscribe(sub_id);
        }

        self.callbacks.clear();
    }
}

impl Default for DataspaceClient {
    fn default() -> Self {
        // Create a default local dataspace for testing
        let ds = Arc::new(RwLock::new(LocalDataspace::new("default")));
        Self::new(ds)
    }
}

/// A subscription handler that collects notifications for later retrieval
#[derive(Debug)]
struct NotifyHandler {
    callback_id: CallbackId,
}

impl NotifyHandler {
    fn new(callback_id: CallbackId) -> Self {
        Self { callback_id }
    }
}

impl sammy::SubscriptionHandler for NotifyHandler {
    fn on_added(&mut self, _handle: Handle, value: &IOValue) {
        // In a real implementation, this would queue a callback to the WASM module
        tracing::debug!(
            callback_id = self.callback_id,
            ?value,
            "Subscription notification: added"
        );
    }

    fn on_removed(&mut self, _handle: Handle, value: &IOValue) {
        tracing::debug!(
            callback_id = self.callback_id,
            ?value,
            "Subscription notification: removed"
        );
    }
}

// =============================================================================
// Legacy compatibility types
// =============================================================================

/// Legacy assertion type for backward compatibility
#[derive(Debug, Clone)]
pub struct Assertion {
    pub id: AssertionId,
    pub pattern: String,
    pub value: Vec<u8>,
}

/// Legacy subscription type for backward compatibility
#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: u64,
    pub pattern: String,
    pub callback: CallbackId,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> DataspaceClient {
        let ds = Arc::new(RwLock::new(LocalDataspace::new("test")));
        DataspaceClient::new(ds)
    }

    #[test]
    fn test_publish_and_query() {
        let mut client = create_test_client();

        let id1 = client.publish("user.login".to_string(), vec![1, 2, 3]).unwrap();
        let id2 = client.publish("user.login".to_string(), vec![4, 5, 6]).unwrap();
        let _id3 = client.publish("user.logout".to_string(), vec![7, 8, 9]).unwrap();

        let results = client.query("user.login");
        assert_eq!(results.len(), 2);

        assert!(id1 > 0);
        assert!(id2 > 0);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_retract() {
        let mut client = create_test_client();

        let id = client.publish("test.pattern".to_string(), vec![1, 2, 3]).unwrap();

        let results = client.query("test.pattern");
        assert_eq!(results.len(), 1);

        client.retract(id).unwrap();

        let results = client.query("test.pattern");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_subscribe() {
        let mut client = create_test_client();

        let sub_id = client.subscribe("user.login".to_string(), 100).unwrap();
        assert!(sub_id > 0);

        client.unsubscribe(sub_id).unwrap();
    }

    #[test]
    fn test_clear() {
        let mut client = create_test_client();

        client.publish("test".to_string(), vec![1, 2, 3]).unwrap();
        client.subscribe("test".to_string(), 100).unwrap();

        assert_eq!(client.assertion_count(), 1);
        assert_eq!(client.subscription_count(), 1);

        client.clear();

        assert_eq!(client.assertion_count(), 0);
        assert_eq!(client.subscription_count(), 0);
    }

    #[test]
    fn test_permissions_block_assert() {
        let ds = Arc::new(RwLock::new(LocalDataspace::new("test")));

        // Create client with observe-only permissions
        let mut client = DataspaceClient::with_permissions(
            ds,
            Permissions::read_only(),
        );

        // Should fail because assert_filter is None (no assertion allowed)
        let result = client.publish("test".to_string(), vec![1, 2, 3]);
        assert!(result.is_err());
    }
}
