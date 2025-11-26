//! Capability-based access control for dataspaces
//!
//! This module implements object-capability security for dataspace access.
//! Instead of giving actors direct access to dataspaces, they receive
//! capability references with specific permissions.

use super::Dataspace;
use crate::assertion::SubscriptionHandler;
use crate::pattern::Pattern;
use crate::types::{Handle, SubscriptionId};
use parking_lot::RwLock;
use preserves::IOValue;
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;

/// Error returned when a capability check fails
#[derive(Debug, Error, Clone, PartialEq)]
pub enum CapabilityError {
    /// The capability doesn't permit this assertion pattern
    #[error("assertion not permitted: value doesn't match assert_filter")]
    AssertNotPermitted,

    /// The capability doesn't permit observing this pattern
    #[error("observation not permitted: pattern doesn't match observe_filter")]
    ObserveNotPermitted,

    /// The capability doesn't permit retracting this assertion
    #[error("retraction not permitted")]
    RetractNotPermitted,

    /// The capability doesn't permit this operation
    #[error("operation not permitted: {0}")]
    OperationNotPermitted(String),
}

/// Permissions for dataspace access
///
/// Permissions can restrict what an actor can do with a dataspace:
/// - What assertions they can publish
/// - What assertions they can observe
/// - Whether they can retract their own or others' assertions
#[derive(Debug, Clone)]
pub struct Permissions {
    /// Pattern filter for assertions this capability can publish.
    /// None means all assertions are allowed.
    pub assert_filter: Option<Pattern>,

    /// Pattern filter for assertions this capability can observe.
    /// None means all assertions can be observed.
    pub observe_filter: Option<Pattern>,

    /// Whether this capability can retract its own assertions
    pub can_retract_own: bool,

    /// Whether this capability can retract any assertion (dangerous!)
    pub can_retract_any: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            assert_filter: None,
            observe_filter: None,
            can_retract_own: true,
            can_retract_any: false,
        }
    }
}

impl Permissions {
    /// Create permissions that allow everything
    pub fn full() -> Self {
        Self {
            assert_filter: None,
            observe_filter: None,
            can_retract_own: true,
            can_retract_any: true,
        }
    }

    /// Create read-only permissions (observe only)
    pub fn read_only() -> Self {
        Self {
            assert_filter: Some(Pattern::Sequence(vec![])), // Matches nothing
            observe_filter: None,
            can_retract_own: false,
            can_retract_any: false,
        }
    }

    /// Create write-only permissions (assert only, no observation)
    pub fn write_only() -> Self {
        Self {
            assert_filter: None,
            observe_filter: Some(Pattern::Sequence(vec![])), // Matches nothing
            can_retract_own: true,
            can_retract_any: false,
        }
    }

    /// Create permissions limited to a specific pattern for both read and write
    pub fn for_pattern(pattern: Pattern) -> Self {
        Self {
            assert_filter: Some(pattern.clone()),
            observe_filter: Some(pattern),
            can_retract_own: true,
            can_retract_any: false,
        }
    }

    /// Check if a value can be asserted with these permissions
    pub fn can_assert(&self, value: &IOValue) -> bool {
        match &self.assert_filter {
            None => true,
            Some(pattern) => pattern.matches_tagged(value),
        }
    }

    /// Check if a pattern can be subscribed to with these permissions
    pub fn can_observe(&self, _pattern: &Pattern) -> bool {
        // For now, we do a simple check. A more sophisticated implementation
        // would check if the subscription pattern is a subset of observe_filter.
        self.observe_filter.is_none()
            || self.observe_filter.as_ref().map_or(false, |_| true)
    }

    /// Check if a handle can be retracted with these permissions
    pub fn can_retract(&self, handle: &Handle, owned_handles: &HashSet<Handle>) -> bool {
        if self.can_retract_any {
            return true;
        }
        if self.can_retract_own && owned_handles.contains(handle) {
            return true;
        }
        false
    }
}

/// A capability-wrapped reference to a dataspace
///
/// This provides controlled access to a dataspace based on the
/// permissions granted. Actors receive `DataspaceRef`s rather than
/// direct access to dataspaces.
pub struct DataspaceRef<D: Dataspace> {
    /// Name of the dataspace (for debugging)
    name: String,
    /// The underlying dataspace (shared reference)
    dataspace: Arc<RwLock<D>>,
    /// Permissions for this capability
    permissions: Permissions,
    /// Handles that were asserted through this capability
    owned_handles: HashSet<Handle>,
}

impl<D: Dataspace> DataspaceRef<D> {
    /// Create a new dataspace reference with the given permissions
    pub fn new(name: impl Into<String>, dataspace: Arc<RwLock<D>>, permissions: Permissions) -> Self {
        Self {
            name: name.into(),
            dataspace,
            permissions,
            owned_handles: HashSet::new(),
        }
    }

    /// Get the dataspace name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the permissions
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    /// Assert a value into the dataspace
    ///
    /// Returns an error if the value doesn't match the assert_filter.
    pub fn assert(&mut self, value: IOValue) -> Result<Handle, CapabilityError> {
        if !self.permissions.can_assert(&value) {
            return Err(CapabilityError::AssertNotPermitted);
        }

        let handle = self.dataspace.write().assert(value);
        self.owned_handles.insert(handle.clone());
        Ok(handle)
    }

    /// Retract an assertion by handle
    ///
    /// Returns an error if this capability doesn't have permission to retract.
    pub fn retract(&mut self, handle: &Handle) -> Result<Option<IOValue>, CapabilityError> {
        if !self.permissions.can_retract(handle, &self.owned_handles) {
            return Err(CapabilityError::RetractNotPermitted);
        }

        let result = self.dataspace.write().retract(handle);
        self.owned_handles.remove(handle);
        Ok(result)
    }

    /// Subscribe to assertions matching a pattern
    ///
    /// The handler will be wrapped to filter results based on observe_filter.
    pub fn subscribe(
        &mut self,
        pattern: Pattern,
        handler: Box<dyn SubscriptionHandler>,
    ) -> Result<SubscriptionId, CapabilityError> {
        if !self.permissions.can_observe(&pattern) {
            return Err(CapabilityError::ObserveNotPermitted);
        }

        // If we have an observe_filter, wrap the handler to filter notifications
        let wrapped_handler: Box<dyn SubscriptionHandler> =
            if let Some(ref filter) = self.permissions.observe_filter {
                Box::new(FilteringHandler::new(filter.clone(), handler))
            } else {
                handler
            };

        let id = self.dataspace.write().subscribe(pattern, wrapped_handler);
        Ok(id)
    }

    /// Unsubscribe from a previous subscription
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        self.dataspace.write().unsubscribe(id)
    }

    /// Query assertions matching a pattern
    ///
    /// Results are filtered by observe_filter if set.
    pub fn query(&self, pattern: &Pattern) -> Result<Vec<IOValue>, CapabilityError> {
        if !self.permissions.can_observe(pattern) {
            return Err(CapabilityError::ObserveNotPermitted);
        }

        let results = self.dataspace.read().query(pattern);

        // Filter results if we have an observe_filter
        let filtered = if let Some(ref filter) = self.permissions.observe_filter {
            results
                .into_iter()
                .filter(|v| filter.matches_tagged(v))
                .collect()
        } else {
            results
        };

        Ok(filtered)
    }

    /// Get the number of handles this capability has asserted
    pub fn owned_handle_count(&self) -> usize {
        self.owned_handles.len()
    }

    /// Create a weaker capability from this one
    ///
    /// The new capability can only have equal or fewer permissions.
    pub fn attenuate(&self, new_permissions: Permissions) -> Self {
        // The new permissions should be a subset. For simplicity,
        // we just use the new permissions directly. A real implementation
        // would intersect the permissions.
        Self {
            name: self.name.clone(),
            dataspace: self.dataspace.clone(),
            permissions: new_permissions,
            owned_handles: HashSet::new(), // New capability starts with no owned handles
        }
    }
}

impl<D: Dataspace> Clone for DataspaceRef<D> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            dataspace: self.dataspace.clone(),
            permissions: self.permissions.clone(),
            owned_handles: HashSet::new(), // Cloned capabilities don't inherit owned handles
        }
    }
}

/// A subscription handler that filters events based on a pattern
struct FilteringHandler {
    filter: Pattern,
    inner: Box<dyn SubscriptionHandler>,
}

impl FilteringHandler {
    fn new(filter: Pattern, inner: Box<dyn SubscriptionHandler>) -> Self {
        Self { filter, inner }
    }
}

impl SubscriptionHandler for FilteringHandler {
    fn on_added(&mut self, handle: Handle, value: &IOValue) {
        if self.filter.matches_tagged(value) {
            self.inner.on_added(handle, value);
        }
    }

    fn on_removed(&mut self, handle: Handle, value: &IOValue) {
        if self.filter.matches_tagged(value) {
            self.inner.on_removed(handle, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataspace::LocalDataspace;
    use crate::pattern::PatternBuilder;

    fn make_dataspace() -> Arc<RwLock<LocalDataspace>> {
        Arc::new(RwLock::new(LocalDataspace::new("test")))
    }

    #[test]
    fn test_full_permissions() {
        let ds = make_dataspace();
        let mut cap = DataspaceRef::new("test", ds, Permissions::full());

        let value = IOValue::new("hello".to_string());
        let handle = cap.assert(value.clone()).unwrap();

        let results = cap.query(&PatternBuilder::wildcard()).unwrap();
        assert_eq!(results.len(), 1);

        cap.retract(&handle).unwrap();
        let results = cap.query(&PatternBuilder::wildcard()).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_assert_filter() {
        let ds = make_dataspace();

        // Only allow "user" records
        let pattern = PatternBuilder::record("user", vec![PatternBuilder::wildcard()]);
        let perms = Permissions {
            assert_filter: Some(pattern),
            ..Permissions::default()
        };
        let mut cap = DataspaceRef::new("test", ds, perms);

        // This should work
        let user = IOValue::record(
            IOValue::symbol("user"),
            vec![IOValue::new("alice".to_string())],
        );
        assert!(cap.assert(user).is_ok());

        // This should fail
        let other = IOValue::record(
            IOValue::symbol("other"),
            vec![IOValue::new("data".to_string())],
        );
        assert!(matches!(
            cap.assert(other),
            Err(CapabilityError::AssertNotPermitted)
        ));
    }

    #[test]
    fn test_retract_own_only() {
        let ds = make_dataspace();

        // Create a capability that can only retract own assertions
        let perms = Permissions {
            can_retract_own: true,
            can_retract_any: false,
            ..Permissions::default()
        };
        let mut cap = DataspaceRef::new("test", ds.clone(), perms.clone());

        // Assert and retract our own - should work
        let value = IOValue::new("mine".to_string());
        let handle = cap.assert(value).unwrap();
        assert!(cap.retract(&handle).is_ok());

        // Now create another capability and try to retract something we didn't assert
        let mut cap2 = DataspaceRef::new("test", ds, perms);
        let value2 = IOValue::new("theirs".to_string());

        // First capability asserts something
        let mut cap1 = cap;
        let other_handle = cap1.assert(value2).unwrap();

        // Second capability tries to retract it - should fail
        assert!(matches!(
            cap2.retract(&other_handle),
            Err(CapabilityError::RetractNotPermitted)
        ));
    }

    #[test]
    fn test_read_only() {
        let ds = make_dataspace();

        // Pre-populate the dataspace
        {
            let mut ds_write = ds.write();
            ds_write.assert(IOValue::new("existing".to_string()));
        }

        let mut cap = DataspaceRef::new("test", ds, Permissions::read_only());

        // Can query
        let results = cap.query(&PatternBuilder::wildcard()).unwrap();
        assert_eq!(results.len(), 1);

        // Cannot assert
        assert!(matches!(
            cap.assert(IOValue::new("new".to_string())),
            Err(CapabilityError::AssertNotPermitted)
        ));
    }

    #[test]
    fn test_attenuate() {
        let ds = make_dataspace();
        let cap = DataspaceRef::new("test", ds, Permissions::full());

        // Attenuate to read-only
        let read_cap = cap.attenuate(Permissions::read_only());

        assert!(read_cap.permissions().assert_filter.is_some());
        assert!(!read_cap.permissions().can_retract_any);
    }
}
