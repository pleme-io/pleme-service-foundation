//! Health check implementation for Kubernetes probes

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Degraded,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub git_sha: String,
    pub uptime_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<serde_json::Value>,
}

/// Trait for health check providers
#[async_trait::async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthStatus;

    fn name(&self) -> &str {
        "default"
    }
}

/// Liveness probe - indicates if service should be restarted
pub struct LivenessProbe {
    start_time: std::time::Instant,
    git_sha: String,
    version: String,
}

impl LivenessProbe {
    pub fn new(git_sha: String, version: String) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            git_sha,
            version,
        }
    }

    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

#[async_trait::async_trait]
impl HealthCheck for LivenessProbe {
    async fn check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        "liveness"
    }
}

/// Readiness probe - indicates if service can accept traffic
#[derive(Clone)]
pub struct ReadinessProbe {
    checks: Vec<Arc<dyn HealthCheck>>,
}

impl ReadinessProbe {
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
        }
    }

    pub fn add_check(mut self, check: Arc<dyn HealthCheck>) -> Self {
        self.checks.push(check);
        self
    }
}

impl Default for ReadinessProbe {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HealthCheck for ReadinessProbe {
    async fn check(&self) -> HealthStatus {
        for check in &self.checks {
            match check.check().await {
                HealthStatus::Unhealthy => return HealthStatus::Unhealthy,
                HealthStatus::Degraded => return HealthStatus::Degraded,
                HealthStatus::Healthy => continue,
            }
        }
        HealthStatus::Healthy
    }

    fn name(&self) -> &str {
        "readiness"
    }
}

/// Axum handler for liveness probe
pub async fn liveness_handler(
    State((liveness, _)): State<(Arc<LivenessProbe>, Arc<ReadinessProbe>)>,
) -> impl IntoResponse {
    let status = liveness.check().await;
    let response = HealthResponse {
        status: status.clone(),
        version: liveness.version.clone(),
        git_sha: liveness.git_sha.clone(),
        uptime_seconds: liveness.uptime(),
        checks: None,
    };

    match status {
        HealthStatus::Healthy => (StatusCode::OK, Json(response)),
        _ => (StatusCode::SERVICE_UNAVAILABLE, Json(response)),
    }
}

/// Axum handler for readiness probe
pub async fn readiness_handler(
    State((liveness, readiness)): State<(Arc<LivenessProbe>, Arc<ReadinessProbe>)>,
) -> impl IntoResponse {
    let status = readiness.check().await;
    let response = HealthResponse {
        status: status.clone(),
        version: liveness.version.clone(),
        git_sha: liveness.git_sha.clone(),
        uptime_seconds: liveness.uptime(),
        checks: None,
    };

    match status {
        HealthStatus::Healthy => (StatusCode::OK, Json(response)),
        _ => (StatusCode::SERVICE_UNAVAILABLE, Json(response)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_liveness_probe() {
        let probe = LivenessProbe::new("abc123".to_string(), "0.1.0".to_string());
        assert_eq!(probe.check().await, HealthStatus::Healthy);
        assert_eq!(probe.name(), "liveness");
    }

    #[tokio::test]
    async fn test_readiness_probe_no_checks() {
        let probe = ReadinessProbe::new();
        assert_eq!(probe.check().await, HealthStatus::Healthy);
    }
}
