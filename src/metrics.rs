//! Service metrics collection

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use serde::Serialize;

/// Service metrics
#[derive(Clone)]
pub struct ServiceMetrics {
    requests_total: Arc<AtomicU64>,
    requests_success: Arc<AtomicU64>,
    requests_error: Arc<AtomicU64>,
}

impl ServiceMetrics {
    pub fn new() -> Self {
        Self {
            requests_total: Arc::new(AtomicU64::new(0)),
            requests_success: Arc::new(AtomicU64::new(0)),
            requests_error: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn record_request(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_success(&self) {
        self.requests_success.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.requests_error.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            requests_success: self.requests_success.load(Ordering::Relaxed),
            requests_error: self.requests_error.load(Ordering::Relaxed),
        }
    }
}

impl Default for ServiceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub requests_total: u64,
    pub requests_success: u64,
    pub requests_error: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics() {
        let metrics = ServiceMetrics::new();

        metrics.record_request();
        metrics.record_success();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.requests_total, 1);
        assert_eq!(snapshot.requests_success, 1);
        assert_eq!(snapshot.requests_error, 0);
    }
}
