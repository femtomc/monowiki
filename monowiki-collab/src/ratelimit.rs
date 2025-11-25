//! Simple per-token rate limiting.
//!
//! Uses a token bucket algorithm with configurable burst and refill rates.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum burst size (bucket capacity)
    pub burst: u32,
    /// Tokens refilled per second
    pub refill_rate: f64,
    /// Whether rate limiting is enabled
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            burst: 10,
            refill_rate: 1.0, // 1 request per second sustained
            enabled: true,
        }
    }
}

/// A token bucket for a single identity
struct Bucket {
    tokens: f64,
    last_refill: Instant,
    capacity: u32,
    refill_rate: f64,
}

impl Bucket {
    fn new(capacity: u32, refill_rate: f64) -> Self {
        Self {
            tokens: capacity as f64,
            last_refill: Instant::now(),
            capacity,
            refill_rate,
        }
    }

    /// Try to consume one token. Returns true if allowed.
    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity as f64);
        self.last_refill = now;
    }

    /// Time until next token is available
    fn time_until_available(&self) -> Duration {
        if self.tokens >= 1.0 {
            Duration::ZERO
        } else {
            let needed = 1.0 - self.tokens;
            Duration::from_secs_f64(needed / self.refill_rate)
        }
    }
}

/// Per-token rate limiter
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    buckets: Arc<RwLock<HashMap<String, Bucket>>>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a request from the given identity is allowed.
    /// Returns Ok(()) if allowed, Err(retry_after) if rate limited.
    pub async fn check(&self, identity: &str) -> Result<(), Duration> {
        if !self.config.enabled {
            return Ok(());
        }

        let mut buckets = self.buckets.write().await;
        let bucket = buckets
            .entry(identity.to_string())
            .or_insert_with(|| Bucket::new(self.config.burst, self.config.refill_rate));

        if bucket.try_consume() {
            Ok(())
        } else {
            Err(bucket.time_until_available())
        }
    }

    /// Clean up old buckets that haven't been used recently.
    /// Call periodically to prevent memory leaks.
    pub async fn cleanup(&self, max_age: Duration) {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();
        buckets.retain(|_, bucket| now.duration_since(bucket.last_refill) < max_age);
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_burst() {
        let limiter = RateLimiter::new(RateLimitConfig {
            burst: 3,
            refill_rate: 1.0,
            enabled: true,
        });

        // Should allow burst of 3
        assert!(limiter.check("test").await.is_ok());
        assert!(limiter.check("test").await.is_ok());
        assert!(limiter.check("test").await.is_ok());

        // 4th request should be rate limited
        assert!(limiter.check("test").await.is_err());
    }

    #[tokio::test]
    async fn test_rate_limiter_separate_identities() {
        let limiter = RateLimiter::new(RateLimitConfig {
            burst: 1,
            refill_rate: 0.1,
            enabled: true,
        });

        // Each identity has separate bucket
        assert!(limiter.check("user1").await.is_ok());
        assert!(limiter.check("user2").await.is_ok());

        // But same identity is limited
        assert!(limiter.check("user1").await.is_err());
    }

    #[tokio::test]
    async fn test_rate_limiter_disabled() {
        let limiter = RateLimiter::new(RateLimitConfig {
            burst: 1,
            refill_rate: 0.0,
            enabled: false,
        });

        // Should always allow when disabled
        for _ in 0..100 {
            assert!(limiter.check("test").await.is_ok());
        }
    }
}
