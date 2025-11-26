//! Query metrics and profiling
//!
//! This module provides instrumentation for tracking query performance
//! and cache effectiveness.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Metrics for a single query type
#[derive(Debug)]
pub struct QueryMetrics {
    /// Name of the query
    pub query_name: &'static str,

    /// Number of cache hits
    pub hit_count: AtomicU64,

    /// Number of cache misses
    pub miss_count: AtomicU64,

    /// Number of early cutoffs (recomputed but value unchanged)
    pub early_cutoff_count: AtomicU64,

    /// Total time spent computing (nanoseconds)
    pub total_compute_time_ns: AtomicU64,

    /// Number of times this query was executed
    pub execution_count: AtomicU64,
}

impl QueryMetrics {
    /// Create new metrics for a query
    pub fn new(query_name: &'static str) -> Self {
        QueryMetrics {
            query_name,
            hit_count: AtomicU64::new(0),
            miss_count: AtomicU64::new(0),
            early_cutoff_count: AtomicU64::new(0),
            total_compute_time_ns: AtomicU64::new(0),
            execution_count: AtomicU64::new(0),
        }
    }

    /// Record a cache hit
    pub fn record_hit(&self) {
        self.hit_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss
    pub fn record_miss(&self) {
        self.miss_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an early cutoff
    pub fn record_early_cutoff(&self) {
        self.early_cutoff_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record query execution time
    pub fn record_execution(&self, duration: Duration) {
        self.execution_count.fetch_add(1, Ordering::Relaxed);
        self.total_compute_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Get cache hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hit_count.load(Ordering::Relaxed) as f64;
        let total = hits + self.miss_count.load(Ordering::Relaxed) as f64;

        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }

    /// Get early cutoff rate (0.0 to 1.0)
    pub fn early_cutoff_rate(&self) -> f64 {
        let cutoffs = self.early_cutoff_count.load(Ordering::Relaxed) as f64;
        let executions = self.execution_count.load(Ordering::Relaxed) as f64;

        if executions == 0.0 {
            0.0
        } else {
            cutoffs / executions
        }
    }

    /// Get average execution time
    pub fn avg_execution_time(&self) -> Duration {
        let total_ns = self.total_compute_time_ns.load(Ordering::Relaxed);
        let count = self.execution_count.load(Ordering::Relaxed);

        if count == 0 {
            Duration::ZERO
        } else {
            Duration::from_nanos(total_ns / count)
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.hit_count.store(0, Ordering::Relaxed);
        self.miss_count.store(0, Ordering::Relaxed);
        self.early_cutoff_count.store(0, Ordering::Relaxed);
        self.total_compute_time_ns.store(0, Ordering::Relaxed);
        self.execution_count.store(0, Ordering::Relaxed);
    }

    /// Get a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            query_name: self.query_name,
            hits: self.hit_count.load(Ordering::Relaxed),
            misses: self.miss_count.load(Ordering::Relaxed),
            early_cutoffs: self.early_cutoff_count.load(Ordering::Relaxed),
            executions: self.execution_count.load(Ordering::Relaxed),
            total_time_ns: self.total_compute_time_ns.load(Ordering::Relaxed),
        }
    }
}

impl Default for QueryMetrics {
    fn default() -> Self {
        Self::new("unknown")
    }
}

/// A point-in-time snapshot of query metrics
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub query_name: &'static str,
    pub hits: u64,
    pub misses: u64,
    pub early_cutoffs: u64,
    pub executions: u64,
    pub total_time_ns: u64,
}

impl MetricsSnapshot {
    /// Get cache hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Get early cutoff rate
    pub fn early_cutoff_rate(&self) -> f64 {
        if self.executions == 0 {
            0.0
        } else {
            self.early_cutoffs as f64 / self.executions as f64
        }
    }

    /// Get average execution time
    pub fn avg_execution_time(&self) -> Duration {
        if self.executions == 0 {
            Duration::ZERO
        } else {
            Duration::from_nanos(self.total_time_ns / self.executions)
        }
    }
}

/// Format metrics snapshot for display
impl std::fmt::Display for MetricsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Query: {}", self.query_name)?;
        writeln!(
            f,
            "  Hits: {} | Misses: {} | Hit Rate: {:.1}%",
            self.hits,
            self.misses,
            self.hit_rate() * 100.0
        )?;
        writeln!(
            f,
            "  Executions: {} | Early Cutoffs: {} | Cutoff Rate: {:.1}%",
            self.executions,
            self.early_cutoffs,
            self.early_cutoff_rate() * 100.0
        )?;
        writeln!(
            f,
            "  Avg Time: {:.2}ms | Total Time: {:.2}ms",
            self.avg_execution_time().as_secs_f64() * 1000.0,
            Duration::from_nanos(self.total_time_ns).as_secs_f64() * 1000.0
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_basic() {
        let metrics = QueryMetrics::new("test_query");

        metrics.record_hit();
        metrics.record_hit();
        metrics.record_miss();

        assert_eq!(metrics.hit_count.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.miss_count.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.hit_rate(), 2.0 / 3.0);
    }

    #[test]
    fn test_metrics_execution_time() {
        let metrics = QueryMetrics::new("test_query");

        metrics.record_execution(Duration::from_millis(10));
        metrics.record_execution(Duration::from_millis(20));

        assert_eq!(metrics.execution_count.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.avg_execution_time(), Duration::from_millis(15));
    }

    #[test]
    fn test_metrics_reset() {
        let metrics = QueryMetrics::new("test_query");

        metrics.record_hit();
        metrics.record_miss();
        metrics.reset();

        assert_eq!(metrics.hit_count.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.miss_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_snapshot() {
        let metrics = QueryMetrics::new("test_query");

        metrics.record_hit();
        metrics.record_miss();
        metrics.record_early_cutoff();

        let snapshot = metrics.snapshot();

        assert_eq!(snapshot.hits, 1);
        assert_eq!(snapshot.misses, 1);
        assert_eq!(snapshot.early_cutoffs, 1);
    }
}
