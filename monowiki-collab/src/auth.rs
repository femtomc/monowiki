//! JWT-based authentication and authorization for monowiki-collab.
//!
//! Supports separate secrets for users vs agents, key rotation via `kid`, and
//! fine-grained capability + slug-glob enforcement.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};
use glob_match::glob_match;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Role distinguishes users from agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Agent,
}

/// Capabilities that can be granted to a token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Read,
    Write,
    Patch,
    Checkpoint,
    Build,
}

impl Capability {
    pub fn all() -> Vec<Capability> {
        vec![
            Capability::Read,
            Capability::Write,
            Capability::Patch,
            Capability::Checkpoint,
            Capability::Build,
        ]
    }
}

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user id or agent id)
    pub sub: String,
    /// Role: user or agent
    pub role: Role,
    /// Allowed slug patterns (glob). None = all slugs allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_slugs: Option<Vec<String>>,
    /// Granted capabilities
    pub capabilities: Vec<Capability>,
    /// Expiry (Unix timestamp)
    pub exp: u64,
    /// Audience (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
}

impl Claims {
    /// Check if this token has a specific capability.
    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Check if a slug is allowed by this token's slug patterns.
    /// Returns true if allowed_slugs is None (all allowed) or if any pattern matches.
    pub fn slug_allowed(&self, slug: &str) -> bool {
        match &self.allowed_slugs {
            None => true,
            Some(patterns) => patterns.iter().any(|pattern| glob_match(pattern, slug)),
        }
    }

    /// Validate that this token can perform `cap` on `slug`.
    pub fn authorize(&self, cap: Capability, slug: Option<&str>) -> Result<(), AuthError> {
        if !self.has_capability(cap) {
            return Err(AuthError::MissingCapability(cap));
        }
        if let Some(s) = slug {
            if !self.slug_allowed(s) {
                return Err(AuthError::SlugNotAllowed(s.to_string()));
            }
        }
        Ok(())
    }
}

/// Errors during authentication/authorization.
#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken(String),
    Expired,
    MissingCapability(Capability),
    SlugNotAllowed(String),
    UnknownKeyId(String),
    RateLimited { retry_after_secs: u64 },
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::MissingToken => write!(f, "missing authorization token"),
            AuthError::InvalidToken(msg) => write!(f, "invalid token: {}", msg),
            AuthError::Expired => write!(f, "token expired"),
            AuthError::MissingCapability(cap) => write!(f, "missing capability: {:?}", cap),
            AuthError::SlugNotAllowed(slug) => write!(f, "slug not allowed: {}", slug),
            AuthError::UnknownKeyId(kid) => write!(f, "unknown key id: {}", kid),
            AuthError::RateLimited { retry_after_secs } => {
                write!(f, "rate limited, retry after {} seconds", retry_after_secs)
            }
        }
    }
}

impl std::error::Error for AuthError {}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match &self {
            AuthError::MissingToken | AuthError::InvalidToken(_) | AuthError::UnknownKeyId(_) => {
                (StatusCode::UNAUTHORIZED, self.to_string()).into_response()
            }
            AuthError::Expired => (StatusCode::UNAUTHORIZED, self.to_string()).into_response(),
            AuthError::MissingCapability(_) | AuthError::SlugNotAllowed(_) => {
                (StatusCode::FORBIDDEN, self.to_string()).into_response()
            }
            AuthError::RateLimited { retry_after_secs } => {
                // Return 429 Too Many Requests with Retry-After header
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    [(
                        axum::http::header::RETRY_AFTER,
                        retry_after_secs.to_string(),
                    )],
                    self.to_string(),
                )
                    .into_response()
            }
        }
    }
}

/// Holds signing keys for JWT verification.
/// Supports multiple keys via `kid` for rotation.
#[derive(Clone)]
pub struct KeyStore {
    /// Map from key id to decoding key
    keys: HashMap<String, DecodingKey>,
    /// Default key id (used when token has no kid)
    default_kid: Option<String>,
    /// Expected audience (optional)
    expected_aud: Option<String>,
    /// Clock skew leeway in seconds (default: 60)
    leeway_secs: u64,
}

impl KeyStore {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            default_kid: None,
            expected_aud: None,
            leeway_secs: 60, // Default 60 second clock skew tolerance
        }
    }

    /// Add a key with the given id and secret.
    pub fn add_key(&mut self, kid: impl Into<String>, secret: impl AsRef<[u8]>) {
        let kid = kid.into();
        let key = DecodingKey::from_secret(secret.as_ref());
        if self.default_kid.is_none() {
            self.default_kid = Some(kid.clone());
        }
        self.keys.insert(kid, key);
    }

    /// Set the default key id for tokens without a kid header.
    pub fn set_default_kid(&mut self, kid: impl Into<String>) {
        self.default_kid = Some(kid.into());
    }

    /// Set expected audience for validation.
    pub fn set_expected_aud(&mut self, aud: impl Into<String>) {
        self.expected_aud = Some(aud.into());
    }

    /// Set clock skew leeway in seconds.
    pub fn set_leeway(&mut self, secs: u64) {
        self.leeway_secs = secs;
    }

    /// Check if this keystore has any keys configured.
    pub fn has_keys(&self) -> bool {
        !self.keys.is_empty()
    }

    /// Decode and validate a JWT token.
    pub fn verify(&self, token: &str) -> Result<Claims, AuthError> {
        // Fail closed if no keys configured
        if self.keys.is_empty() {
            return Err(AuthError::InvalidToken("no keys configured".into()));
        }

        // First decode header to get kid
        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| AuthError::InvalidToken(e.to_string()))?;

        let kid = header
            .kid
            .or_else(|| self.default_kid.clone())
            .ok_or_else(|| AuthError::InvalidToken("no key id and no default key".into()))?;

        let key = self
            .keys
            .get(&kid)
            .ok_or_else(|| AuthError::UnknownKeyId(kid.clone()))?;

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.leeway = self.leeway_secs;

        if let Some(ref aud) = self.expected_aud {
            validation.set_audience(&[aud]);
        } else {
            validation.validate_aud = false;
        }

        let token_data = decode::<Claims>(token, key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::Expired,
                jsonwebtoken::errors::ErrorKind::InvalidAudience => {
                    AuthError::InvalidToken("audience mismatch".into())
                }
                _ => AuthError::InvalidToken(e.to_string()),
            })?;

        // Verify kid matches expected role
        // User tokens must use "user" kid, agent tokens must use "agent" kid
        let expected_kid = match token_data.claims.role {
            Role::User => "user",
            Role::Agent => "agent",
        };
        if kid != expected_kid {
            return Err(AuthError::InvalidToken(format!(
                "token role {:?} does not match key id '{}'",
                token_data.claims.role, kid
            )));
        }

        Ok(token_data.claims)
    }
}

impl Default for KeyStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for authentication.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Secret for user tokens (env: MONOWIKI_USER_SECRET)
    pub user_secret: Option<String>,
    /// Secret for agent tokens (env: MONOWIKI_AGENT_SECRET)
    pub agent_secret: Option<String>,
    /// Expected audience (optional)
    pub expected_aud: Option<String>,
    /// Whether auth is required (false = allow unauthenticated)
    pub require_auth: bool,
}

impl AuthConfig {
    /// Build a KeyStore from this config.
    pub fn build_keystore(&self) -> KeyStore {
        let mut store = KeyStore::new();

        if let Some(ref secret) = self.user_secret {
            store.add_key("user", secret.as_bytes());
        }
        if let Some(ref secret) = self.agent_secret {
            store.add_key("agent", secret.as_bytes());
        }

        // Default to user key if only one is set
        if self.user_secret.is_some() && self.agent_secret.is_none() {
            store.set_default_kid("user");
        } else if self.agent_secret.is_some() && self.user_secret.is_none() {
            store.set_default_kid("agent");
        }

        if let Some(ref aud) = self.expected_aud {
            store.set_expected_aud(aud);
        }

        store
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            user_secret: None,
            agent_secret: None,
            expected_aud: None,
            require_auth: false,
        }
    }
}

/// Wrapper for optional authenticated claims (used in routes that work with or without auth)
#[derive(Debug, Clone)]
pub struct MaybeClaims(pub Option<Claims>);

/// Wrapper for required authenticated claims
#[derive(Debug, Clone)]
pub struct AuthenticatedClaims(pub Claims);

/// State extension that holds auth config
#[derive(Clone)]
pub struct AuthState {
    pub keystore: Arc<KeyStore>,
    pub require_auth: bool,
}

/// Extract bearer token from Authorization header
fn extract_bearer_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

/// Extractor for optional claims (doesn't fail if no token)
impl<S> FromRequestParts<S> for MaybeClaims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try to get AuthState from app state via extension
        let auth_state = parts.extensions.get::<AuthState>().cloned();

        let Some(auth_state) = auth_state else {
            // No auth configured, allow through
            return Ok(MaybeClaims(None));
        };

        let Some(token) = extract_bearer_token(parts) else {
            if auth_state.require_auth {
                return Err(AuthError::MissingToken);
            }
            return Ok(MaybeClaims(None));
        };

        match auth_state.keystore.verify(token) {
            Ok(claims) => Ok(MaybeClaims(Some(claims))),
            Err(e) => {
                warn!("auth failed: {}", e);
                Err(e)
            }
        }
    }
}

/// Extractor for required claims (fails if no valid token)
impl<S> FromRequestParts<S> for AuthenticatedClaims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_state = parts.extensions.get::<AuthState>().cloned();

        let Some(auth_state) = auth_state else {
            // No auth configured - if we're requiring auth and there's no state, reject
            return Err(AuthError::MissingToken);
        };

        let token = extract_bearer_token(parts).ok_or(AuthError::MissingToken)?;

        let claims = auth_state.keystore.verify(token)?;
        Ok(AuthenticatedClaims(claims))
    }
}

/// Helper to create a full-access user token for testing/CLI
pub fn create_user_token(
    secret: &[u8],
    sub: &str,
    expires_in_secs: u64,
) -> Result<String> {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + expires_in_secs;

    let claims = Claims {
        sub: sub.to_string(),
        role: Role::User,
        allowed_slugs: None,
        capabilities: Capability::all(),
        exp,
        aud: None,
    };

    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("user".to_string());

    encode(&header, &claims, &EncodingKey::from_secret(secret))
        .map_err(|e| anyhow!("failed to encode token: {}", e))
}

/// Helper to create a scoped agent token
pub fn create_agent_token(
    secret: &[u8],
    sub: &str,
    allowed_slugs: Vec<String>,
    capabilities: Vec<Capability>,
    expires_in_secs: u64,
) -> Result<String> {
    use jsonwebtoken::{encode, EncodingKey, Header};

    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + expires_in_secs;

    let claims = Claims {
        sub: sub.to_string(),
        role: Role::Agent,
        allowed_slugs: Some(allowed_slugs),
        capabilities,
        exp,
        aud: None,
    };

    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some("agent".to_string());

    encode(&header, &claims, &EncodingKey::from_secret(secret))
        .map_err(|e| anyhow!("failed to encode token: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_glob_matching() {
        let claims = Claims {
            sub: "test".into(),
            role: Role::Agent,
            allowed_slugs: Some(vec!["drafts/**".into(), "notes/ai-*".into()]),
            capabilities: vec![Capability::Read],
            exp: u64::MAX,
            aud: None,
        };

        assert!(claims.slug_allowed("drafts/foo"));
        assert!(claims.slug_allowed("drafts/bar/baz")); // ** matches nested
        assert!(claims.slug_allowed("notes/ai-thoughts"));
        assert!(!claims.slug_allowed("notes/human-thoughts"));
        assert!(!claims.slug_allowed("private/secret"));
    }

    #[test]
    fn test_user_full_access() {
        let claims = Claims {
            sub: "user".into(),
            role: Role::User,
            allowed_slugs: None,
            capabilities: Capability::all(),
            exp: u64::MAX,
            aud: None,
        };

        assert!(claims.slug_allowed("anything/at/all"));
        assert!(claims.has_capability(Capability::Read));
        assert!(claims.has_capability(Capability::Write));
        assert!(claims.has_capability(Capability::Checkpoint));
    }

    #[test]
    fn test_token_roundtrip() {
        let secret = b"test-secret";
        let token = create_user_token(secret, "testuser", 3600).unwrap();

        let mut store = KeyStore::new();
        store.add_key("user", secret);

        let claims = store.verify(&token).unwrap();
        assert_eq!(claims.sub, "testuser");
        assert_eq!(claims.role, Role::User);
        assert!(claims.allowed_slugs.is_none());
    }

    #[test]
    fn test_agent_token_scoped() {
        let secret = b"agent-secret";
        let token = create_agent_token(
            secret,
            "gpt-assistant",
            vec!["drafts/*".into()],
            vec![Capability::Read, Capability::Patch],
            3600,
        )
        .unwrap();

        let mut store = KeyStore::new();
        store.add_key("agent", secret);

        let claims = store.verify(&token).unwrap();
        assert_eq!(claims.sub, "gpt-assistant");
        assert_eq!(claims.role, Role::Agent);
        assert!(claims.has_capability(Capability::Read));
        assert!(claims.has_capability(Capability::Patch));
        assert!(!claims.has_capability(Capability::Write));
        assert!(claims.slug_allowed("drafts/test"));
        assert!(!claims.slug_allowed("private/note"));
    }
}
