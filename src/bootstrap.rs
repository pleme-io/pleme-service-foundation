//! Service bootstrap and initialization

use crate::{GracefulShutdown, LivenessProbe, ReadinessProbe, ServiceError, Result};
use axum::{Router, routing::get};
use pleme_config::RunMode;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{
    compression::CompressionLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::info;
use std::time::Duration;

/// Type alias for async task functions
pub type AsyncTask = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// Type alias for health check functions
pub type HealthCheckFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;

/// Service information
#[derive(Clone, Debug)]
pub struct ServiceInfo {
    pub name: String,
    pub version: String,
    pub git_sha: String,
}

/// Service builder for standardized initialization
pub struct ServiceBuilder {
    info: ServiceInfo,
    port: u16,
    run_mode: RunMode,
    router: Option<Router>,
    readiness: ReadinessProbe,
    migrate_task: Option<AsyncTask>,
    worker_task: Option<AsyncTask>,
    promote_task: Option<AsyncTask>,
}

impl ServiceBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        let git_sha = std::env::var("GIT_SHA").unwrap_or_else(|_| "unknown".to_string());
        let version = std::env::var("VERSION").unwrap_or_else(|_| "0.1.0".to_string());
        let run_mode_str = std::env::var("RUN_MODE").unwrap_or_else(|_| "api".to_string());
        let run_mode = run_mode_str.parse().unwrap_or(RunMode::Api);

        Self {
            info: ServiceInfo {
                name: name.into(),
                version,
                git_sha,
            },
            port: 8080,
            run_mode,
            router: None,
            readiness: ReadinessProbe::new(),
            migrate_task: None,
            worker_task: None,
            promote_task: None,
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_run_mode(mut self, mode: RunMode) -> Self {
        self.run_mode = mode;
        self
    }

    pub fn with_router(mut self, router: Router) -> Self {
        self.router = Some(router);
        self
    }

    pub fn with_readiness_check(mut self, check: Arc<dyn crate::HealthCheck>) -> Self {
        self.readiness = self.readiness.add_check(check);
        self
    }

    /// Add a custom health check using a closure
    ///
    /// # Example
    /// ```rust,no_run
    /// use pleme_service_foundation::ServiceBuilder;
    /// use std::sync::Arc;
    ///
    /// ServiceBuilder::new("my-service")
    ///     .with_health_check_fn(Arc::new(|| {
    ///         Box::pin(async {
    ///             // Check database connection
    ///             true
    ///         })
    ///     }));
    /// ```
    pub fn with_health_check_fn(self, _check: HealthCheckFn) -> Self {
        // TODO: Implement closure-based health checks
        self
    }

    /// Set migration task for migrate mode
    ///
    /// # Example
    /// ```rust,no_run
    /// use pleme_service_foundation::ServiceBuilder;
    ///
    /// ServiceBuilder::new("my-service")
    ///     .with_migrate_task(Box::pin(async {
    ///         // Run migrations
    ///         Ok(())
    ///     }));
    /// ```
    pub fn with_migrate_task(mut self, task: AsyncTask) -> Self {
        self.migrate_task = Some(task);
        self
    }

    /// Set worker task for worker mode
    pub fn with_worker_task(mut self, task: AsyncTask) -> Self {
        self.worker_task = Some(task);
        self
    }

    /// Set promote task for promote mode
    pub fn with_promote_task(mut self, task: AsyncTask) -> Self {
        self.promote_task = Some(task);
        self
    }

    /// Build application router with health endpoints
    fn build_app(self, router: Option<Router>) -> Router {
        let liveness = Arc::new(LivenessProbe::new(
            self.info.git_sha.clone(),
            self.info.version.clone(),
        ));
        let readiness = Arc::new(self.readiness.clone());
        let state = (liveness, readiness);

        // Create health router with state
        let health_router = Router::new()
            .route("/health", get(crate::health::liveness_handler))
            .route("/ready", get(crate::health::readiness_handler))
            .with_state(state);

        // Merge with application router if provided
        let app = if let Some(r) = router {
            r.merge(health_router)
        } else {
            health_router
        };

        app
            .layer(CompressionLayer::new())
            .layer(TimeoutLayer::new(Duration::from_secs(30)))
            .layer(TraceLayer::new_for_http())
    }

    /// Run the service (dispatches to appropriate mode)
    pub async fn run(self) -> Result<()> {
        // Initialize logging first
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        info!(
            service = %self.info.name,
            version = %self.info.version,
            git_sha = %self.info.git_sha,
            run_mode = %self.run_mode,
            "Starting service"
        );

        // Dispatch to appropriate run method based on mode
        match self.run_mode {
            RunMode::Api => self.run_api().await,
            RunMode::Migrate => self.run_migrate().await,
            RunMode::Worker => self.run_worker().await,
            RunMode::Promote => self.run_promote().await,
        }
    }

    /// Run in API mode - serve HTTP/GraphQL requests
    async fn run_api(self) -> Result<()> {
        let ServiceBuilder { info, port, router, readiness, .. } = self;

        // Build app with router
        let builder = ServiceBuilder {
            info: info.clone(),
            port,
            run_mode: RunMode::Api,
            router: None,
            readiness,
            migrate_task: None,
            worker_task: None,
            promote_task: None,
        };
        let app = builder.build_app(router);

        let addr = SocketAddr::from(([0, 0, 0, 0], port));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ServiceError::BindError {
                port,
                source: e,
            })?;

        info!("Listening on {} in API mode", addr);

        let shutdown = GracefulShutdown::new().listen_for_signals();

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let mut rx = shutdown.subscribe();
                let _ = rx.recv().await;
                info!("Shutdown signal received, stopping server");
            })
            .await
            .map_err(|e| ServiceError::InitializationFailed(e.to_string()))?;

        info!("Service stopped gracefully");
        Ok(())
    }

    /// Run in migrate mode - execute database migrations and exit
    async fn run_migrate(self) -> Result<()> {
        info!("Running in migrate mode");

        if let Some(task) = self.migrate_task {
            task.await?;
            info!("Migrations completed successfully");
        } else {
            return Err(ServiceError::InitializationFailed(
                "No migration task configured".to_string(),
            ));
        }

        Ok(())
    }

    /// Run in worker mode - process background jobs
    async fn run_worker(self) -> Result<()> {
        info!("Running in worker mode");

        if let Some(task) = self.worker_task {
            let shutdown = GracefulShutdown::new().listen_for_signals();
            let mut rx = shutdown.subscribe();

            tokio::select! {
                result = task => {
                    result?;
                    info!("Worker task completed");
                }
                _ = rx.recv() => {
                    info!("Shutdown signal received, stopping worker");
                }
            }
        } else {
            return Err(ServiceError::InitializationFailed(
                "No worker task configured".to_string(),
            ));
        }

        Ok(())
    }

    /// Run in promote mode - promote schema changes for zero-downtime deployments
    async fn run_promote(self) -> Result<()> {
        info!("Running in promote mode");

        if let Some(task) = self.promote_task {
            task.await?;
            info!("Promotion completed successfully");
        } else {
            return Err(ServiceError::InitializationFailed(
                "No promote task configured".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_builder() {
        let builder = ServiceBuilder::new("test-service")
            .with_port(9000);

        assert_eq!(builder.info.name, "test-service");
        assert_eq!(builder.port, 9000);
    }
}
