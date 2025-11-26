//! Sammy - Syndicated Actor Model runtime for Monowiki
//!
//! This crate provides a modular, trait-based syndicated actor runtime.
//! It's designed to be generic and customizable, allowing `monowiki-runtime`
//! to plug in domain-specific implementations.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        Runtime<C>                           │
//! │  (Coordinator - manages dataspaces and actors)              │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                             │
//! │  ┌─────────────────┐  ┌─────────────────┐                   │
//! │  │   Dataspace A   │  │   Dataspace B   │  ...              │
//! │  │ (doc-content/x) │  │ (doc-view/x)    │                   │
//! │  └────────┬────────┘  └────────┬────────┘                   │
//! │           │                    │                            │
//! │           │    Capabilities    │                            │
//! │           │   (Permissions)    │                            │
//! │           ▼                    ▼                            │
//! │  ┌─────────────────────────────────────────────────────┐   │
//! │  │                      Actors                          │   │
//! │  │  ┌──────────────┐  ┌──────────────┐                  │   │
//! │  │  │   Actor 1    │  │   Actor 2    │  ...             │   │
//! │  │  │  ┌────────┐  │  │  ┌────────┐  │                  │   │
//! │  │  │  │ Facets │  │  │  │ Facets │  │                  │   │
//! │  │  │  │┌──────┐│  │  │  │┌──────┐│  │                  │   │
//! │  │  │  ││Entity││  │  │  ││Entity││  │                  │   │
//! │  │  │  │└──────┘│  │  │  │└──────┘│  │                  │   │
//! │  │  │  └────────┘  │  │  └────────┘  │                  │   │
//! │  │  └──────────────┘  └──────────────┘                  │   │
//! │  └─────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Core Concepts
//!
//! ## Dataspaces
//!
//! A **dataspace** is a named container for assertions. Actors interact by:
//! - Publishing assertions (state they want others to see)
//! - Subscribing to patterns (receiving notifications)
//! - Querying current state
//!
//! ## Actors and Facets
//!
//! An **actor** is an independent computational entity. Each actor contains
//! **facets** - units of isolation that provide structured concurrency.
//! When a facet stops, all its children stop and its assertions are retracted.
//!
//! ## Entities
//!
//! An **entity** implements actor behavior via the `Entity` trait:
//! - `on_assert`: Called when matching assertions appear
//! - `on_retract`: Called when matching assertions are removed
//! - `on_message`: Called for direct messages
//! - `on_stop`: Called when the facet is stopping
//!
//! ## Capabilities
//!
//! **Capabilities** are unforgeable references that grant controlled access
//! to dataspaces. Permissions can restrict what an actor can assert, observe,
//! or retract.
//!
//! # Customization Points
//!
//! The crate uses traits for key extension points:
//!
//! - `AssertionStore`: Storage backend for assertions (default: OR-Set)
//! - `Dataspace`: Container abstraction (default: `LocalDataspace`)
//! - `Entity<D>`: Actor behavior implementation
//! - `RuntimeConfig`: Configuration for creating dataspaces
//!
//! # Example
//!
//! ```rust
//! use sammy::runtime::{Runtime, DefaultConfig};
//! use sammy::dataspace::{Dataspace, Permissions};
//! use sammy::pattern::PatternBuilder;
//! use preserves::IOValue;
//!
//! // Create a runtime
//! let mut runtime = Runtime::with_defaults();
//!
//! // Create a dataspace
//! let ds = runtime.dataspace("my-space");
//!
//! // Spawn an actor and grant it a capability
//! let actor_id = runtime.spawn_actor("my-actor");
//! runtime.grant_capability(&actor_id, "my-space", Permissions::full());
//!
//! // Use the capability to assert something
//! {
//!     let actor = runtime.actor_mut(&actor_id).unwrap();
//!     let cap = actor.capability_mut("my-space").unwrap();
//!     let value = IOValue::record(
//!         IOValue::symbol("greeting"),
//!         vec![IOValue::new("hello".to_string())]
//!     );
//!     cap.assert(value).unwrap();
//! }
//!
//! // Query the dataspace
//! let pattern = PatternBuilder::record("greeting", vec![PatternBuilder::wildcard()]);
//! let results = runtime.dataspace("my-space").read().query(&pattern);
//! assert_eq!(results.len(), 1);
//! ```

// Modules
pub mod actor;
pub mod assertion;
pub mod dataspace;
pub mod pattern;
pub mod runtime;
pub mod types;

// Re-exports for convenience
pub use actor::{Actor, CollectingEntity, Entity, EntityContext, Facet, LoggingEntity, NoopEntity};
pub use assertion::{AssertionEvent, AssertionStore, OrSetStore, SubscriptionHandler};
pub use dataspace::{CapabilityError, Dataspace, DataspaceRef, LocalDataspace, Permissions};
pub use pattern::{BindingValue, MatchDatum, Pattern, PatternBuilder};
pub use runtime::{DefaultConfig, Runtime, RuntimeConfig, RuntimeStats};
pub use types::{ActorId, FacetId, Handle, SubscriptionId, TurnId};
