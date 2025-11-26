//! Assertion storage and subscription primitives
//!
//! This module provides the core abstractions for storing assertions
//! and notifying subscribers of changes.

mod store;
mod subscription;

pub use store::{AssertionStore, OrSetStore};
pub use subscription::{
    AddOnlyHandler, AssertionEvent, CallbackHandler, CollectingHandler, SubscriptionHandler,
    SubscriptionManager,
};
