//! # Types
//!
//! Core types for the reconciler.

use crate::controller::backoff::FibonacciBackoff;
use anyhow::Result;
use kube::Client;
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use thiserror::Error;
use tokio::sync::Mutex as AsyncMutex;

#[derive(Debug, Error)]
pub enum ReconcilerError {
    #[error("Reconciliation failed: {0}")]
    ReconciliationFailed(#[from] anyhow::Error),
}

/// Trigger source for reconciliation
/// Tracks why a reconciliation was triggered for better debugging and observability
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerSource {
    /// Manual trigger via CLI annotation (msmctl reconcile)
    ManualCli,
    /// Timer-based periodic reconciliation (reconcile_interval)
    TimerBased,
    /// Error backoff retry (Fibonacci backoff after failure)
    ErrorBackoff,
    /// Waiting for resource (GitRepository 404)
    WaitingForResource,
    /// Retry after error (generic retry)
    RetryAfterError,
}

impl TriggerSource {
    /// Get human-readable string representation
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            TriggerSource::ManualCli => "manual-cli",
            TriggerSource::TimerBased => "timer-based",
            TriggerSource::ErrorBackoff => "error-backoff",
            TriggerSource::WaitingForResource => "waiting-for-resource",
            TriggerSource::RetryAfterError => "retry-after-error",
        }
    }
}

/// Backoff state for a specific resource
/// Tracks error count and backoff calculator for progressive retries
#[derive(Debug, Clone)]
pub struct BackoffState {
    pub backoff: FibonacciBackoff,
    pub error_count: u32,
}

impl BackoffState {
    pub fn new() -> Self {
        Self {
            backoff: FibonacciBackoff::new(1, 10), // 1 minute min, 10 minutes max (converted to seconds internally)
            error_count: 0,
        }
    }

    pub fn increment_error(&mut self) {
        self.error_count += 1;
    }

    pub fn reset(&mut self) {
        self.error_count = 0;
        self.backoff.reset();
    }
}

#[derive(Clone)]
pub struct Reconciler {
    pub client: Client,
    // Note: secret_manager is created per-reconciliation to support per-resource auth config
    // In the future, we might want to cache clients per auth config
    // SOPS private key is wrapped in Arc<AsyncMutex> to allow hot-reloading when secret changes
    pub sops_private_key: Arc<AsyncMutex<Option<String>>>,
    // SOPS capability bootstrap flag - tracks if SOPS is configured globally (controller namespace)
    // Set to true once key is successfully loaded, updated by watch when key changes
    // This separates "system readiness" from "per-file decryption" concerns
    pub sops_capability_ready: Arc<AtomicBool>,
    // Backoff state per resource (identified by namespace/name)
    // Moved to error_policy() layer to prevent blocking watch/timer paths
    pub backoff_states: Arc<Mutex<HashMap<String, BackoffState>>>,
    // Git operation locks per resource (identified by namespace/name)
    // Ensures only one git operation (clone/fetch) per resource at a time
    // Uses AsyncMutex to serialize git operations without blocking the entire controller
    pub git_operation_locks: Arc<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
}

impl std::fmt::Debug for Reconciler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Note: We can't lock the mutex in Debug, so we just indicate if it's set
        f.debug_struct("Reconciler")
            .field("sops_private_key", &"***")
            .finish_non_exhaustive()
    }
}

impl Reconciler {
    pub async fn new(client: Client) -> Result<Self> {
        // Provider is created per-reconciliation based on provider config
        // Per-resource auth config is handled in reconcile()

        // Load SOPS private key from Kubernetes secret
        let sops_private_key =
            crate::controller::reconciler::sops::load_sops_private_key(&client).await?;

        // Set capability flag based on whether key was loaded
        // This proves "SOPS is configured and ready" at bootstrap time
        let sops_capability_ready = Arc::new(AtomicBool::new(sops_private_key.is_some()));

        if sops_private_key.is_some() {
            tracing::info!("✅ SOPS capability ready - GPG key loaded from controller namespace");
        } else {
            tracing::warn!("⚠️  SOPS capability not ready - no key in controller namespace");
            tracing::warn!("   SOPS decryption will be disabled until key is added");
        }

        Ok(Self {
            client,
            sops_private_key: Arc::new(AsyncMutex::new(sops_private_key)),
            sops_capability_ready,
            backoff_states: Arc::new(Mutex::new(HashMap::new())),
            git_operation_locks: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Get or create a git operation lock for a resource
    /// This ensures only one git operation (clone/fetch) per resource at a time
    /// Returns a guard that will be released when dropped
    pub fn get_git_operation_lock(&self, namespace: &str, name: &str) -> Arc<AsyncMutex<()>> {
        let resource_key = format!("{}/{}", namespace, name);
        let mut locks = self.git_operation_locks.lock().unwrap();
        locks
            .entry(resource_key)
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }
}
