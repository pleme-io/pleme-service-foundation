//! # pleme-service-foundation
//!
//! Service foundation library providing common infrastructure for Pleme microservices.
//!
//! ## Features
//!
//! - **Service Bootstrap** - Standard initialization patterns
//! - **Health Checks** - Kubernetes-compatible liveness/readiness probes
//! - **Graceful Shutdown** - Signal handling and cleanup
//! - **Metrics** - Built-in service metrics
//!
//! ## Usage
//!
//! ```rust,no_run
//! use pleme_service_foundation::{ServiceBuilder, HealthCheck};
//! use axum::{Router, routing::get};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let app = Router::new()
//!         .route("/api/users", get(|| async { "users" }));
//!
//!     ServiceBuilder::new("my-service")
//!         .with_port(8080)
//!         .with_router(app)
//!         .run()
//!         .await
//! }
//! ```

pub mod health;
pub mod shutdown;
pub mod bootstrap;
pub mod metrics;

pub use health::{HealthCheck, HealthStatus, LivenessProbe, ReadinessProbe};
pub use shutdown::{GracefulShutdown, ShutdownSignal};
pub use bootstrap::{ServiceBuilder, ServiceInfo, AsyncTask, HealthCheckFn};
pub use metrics::ServiceMetrics;
pub use pleme_config::RunMode;

use thiserror::Error;

/// Service foundation errors
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Failed to bind to port {port}: {source}")]
    BindError {
        port: u16,
        #[source]
        source: std::io::Error,
    },

    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Service initialization failed: {0}")]
    InitializationFailed(String),
}

/// Result type for service operations
pub type Result<T> = std::result::Result<T, ServiceError>;
