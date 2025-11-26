//! Dataspace abstraction for assertion-based communication
//!
//! A dataspace is a named container for assertions where actors can:
//! - Publish assertions (values that exist as long as they're maintained)
//! - Subscribe to patterns (receive notifications when matching assertions change)
//! - Query current assertions
//!
//! This module provides:
//! - `Dataspace` trait: The core abstraction
//! - `LocalDataspace`: Default single-process implementation
//! - Capability system for controlled access

mod capability;
mod local;

pub use capability::{CapabilityError, DataspaceRef, Permissions};
pub use local::LocalDataspace;

use crate::assertion::{AssertionStore, SubscriptionHandler};
use crate::pattern::Pattern;
use crate::types::{Handle, SubscriptionId};
use preserves::IOValue;

/// Core trait for dataspace implementations
///
/// A dataspace is a named, shared assertion space. Actors interact by:
/// - Publishing assertions (state they want others to see)
/// - Subscribing to patterns (receiving notifications)
/// - Querying current state
///
/// Different implementations can provide:
/// - Local (single-process) dataspaces
/// - Distributed dataspaces (across network)
/// - Persistent dataspaces (surviving restarts)
pub trait Dataspace: Send + Sync {
    /// The underlying storage type
    type Store: AssertionStore;

    /// Get the dataspace name
    fn name(&self) -> &str;

    /// Get a reference to the underlying store
    fn store(&self) -> &Self::Store;

    /// Get a mutable reference to the underlying store
    fn store_mut(&mut self) -> &mut Self::Store;

    /// Assert a value into the dataspace
    ///
    /// Returns a handle that can be used to later retract the assertion.
    /// The assertion remains active until explicitly retracted.
    fn assert(&mut self, value: IOValue) -> Handle;

    /// Retract an assertion by handle
    ///
    /// Returns the value if it was present, None otherwise.
    fn retract(&mut self, handle: &Handle) -> Option<IOValue>;

    /// Subscribe to assertions matching a pattern
    ///
    /// The handler will be called for:
    /// - All existing assertions that match (on_added)
    /// - Future assertions that match (on_added when added, on_removed when removed)
    fn subscribe(
        &mut self,
        pattern: Pattern,
        handler: Box<dyn SubscriptionHandler>,
    ) -> SubscriptionId;

    /// Unsubscribe from a previous subscription
    fn unsubscribe(&mut self, id: SubscriptionId) -> bool;

    /// Query all assertions matching a pattern
    ///
    /// Returns current matches without establishing a subscription.
    fn query(&self, pattern: &Pattern) -> Vec<IOValue>;

    /// Get all assertion handles and values
    fn assertions(&self) -> Vec<(Handle, IOValue)> {
        self.store().iter().map(|(h, v)| (h.clone(), v.clone())).collect()
    }

    /// Get the number of active assertions
    fn assertion_count(&self) -> usize {
        self.store().len()
    }
}
