//! Graceful shutdown handling

use tokio::sync::broadcast;
use tracing::{info, warn};

/// Signal for graceful shutdown
#[derive(Clone, Debug)]
pub struct ShutdownSignal;

/// Graceful shutdown coordinator
pub struct GracefulShutdown {
    tx: broadcast::Sender<ShutdownSignal>,
}

impl GracefulShutdown {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1);
        Self { tx }
    }

    /// Subscribe to shutdown signal
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownSignal> {
        self.tx.subscribe()
    }

    /// Trigger shutdown
    pub fn shutdown(&self) {
        info!("Triggering graceful shutdown");
        let _ = self.tx.send(ShutdownSignal);
    }

    /// Wait for OS signals (SIGTERM, SIGINT)
    pub async fn wait_for_signal(&self) {
        use signal_hook::consts::signal::*;
        use signal_hook_tokio::Signals;
        use futures::stream::StreamExt;

        let signals = Signals::new(&[SIGTERM, SIGINT, SIGQUIT])
            .expect("Failed to create signal handler");

        let mut signals_stream = signals.fuse();

        tokio::spawn(async move {
            if let Some(signal) = signals_stream.next().await {
                match signal {
                    SIGTERM => info!("Received SIGTERM"),
                    SIGINT => info!("Received SIGINT"),
                    SIGQUIT => info!("Received SIGQUIT"),
                    _ => warn!("Received unknown signal: {}", signal),
                }
            }
        });

        let mut rx = self.subscribe();
        let _ = rx.recv().await;
    }

    /// Start signal listener in background
    pub fn listen_for_signals(self) -> Self {
        let shutdown = self.clone();
        tokio::spawn(async move {
            use signal_hook::consts::signal::*;
            use signal_hook_tokio::Signals;
            use futures::stream::StreamExt;

            let signals = Signals::new(&[SIGTERM, SIGINT, SIGQUIT])
                .expect("Failed to create signal handler");

            let mut signals_stream = signals.fuse();

            if let Some(signal) = signals_stream.next().await {
                match signal {
                    SIGTERM => info!("Received SIGTERM, initiating graceful shutdown"),
                    SIGINT => info!("Received SIGINT, initiating graceful shutdown"),
                    SIGQUIT => info!("Received SIGQUIT, initiating graceful shutdown"),
                    _ => warn!("Received unknown signal: {}, initiating shutdown", signal),
                }
                shutdown.shutdown();
            }
        });

        self
    }
}

impl Default for GracefulShutdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for GracefulShutdown {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_signal() {
        let shutdown = GracefulShutdown::new();
        let mut rx = shutdown.subscribe();

        shutdown.shutdown();

        let result = rx.recv().await;
        assert!(result.is_ok());
    }
}
