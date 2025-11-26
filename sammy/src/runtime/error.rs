//! Error types for the Sammy runtime

use thiserror::Error;
use uuid::Uuid;

/// Top-level runtime error
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Actor-related errors
    #[error("Actor error: {0}")]
    Actor(#[from] ActorError),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Initialization errors
    #[error("Initialization failed: {0}")]
    Init(String),

    /// Pattern matching errors
    #[error("Pattern error: {0}")]
    Pattern(String),
}

/// Actor execution errors
#[derive(Debug, Error)]
pub enum ActorError {
    /// Actor not found
    #[error("Actor {0} not found")]
    NotFound(String),

    /// Facet not found
    #[error("Facet {0} not found")]
    FacetNotFound(String),

    /// Entity not found
    #[error("Entity {0} not found")]
    EntityNotFound(Uuid),

    /// Invalid activation
    #[error("Invalid activation: {0}")]
    InvalidActivation(String),

    /// Turn execution failed
    #[error("Turn execution failed: {0}")]
    ExecutionFailed(String),
}

/// Result type using RuntimeError
pub type Result<T> = std::result::Result<T, RuntimeError>;

/// Result type using ActorError
pub type ActorResult<T> = std::result::Result<T, ActorError>;
