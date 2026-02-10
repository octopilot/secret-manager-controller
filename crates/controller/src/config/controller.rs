//! # Controller Configuration
//!
//! Controller-level settings loaded from environment variables.

use std::time::Duration;

/// Controller-level configuration
///
/// All settings have sensible defaults and can be overridden via environment variables.
/// Environment variables are populated from a ConfigMap using `envFrom` in the deployment.
#[derive(Debug, Clone)]
pub struct ControllerConfig {
    /// Reconciliation error requeue interval (seconds)
    /// How long to wait before retrying a failed reconciliation
    pub reconciliation_error_requeue_secs: u64,
    /// Exponential backoff starting value (milliseconds)
    /// Initial delay before retrying after an error
    pub backoff_start_ms: u64,
    /// Exponential backoff maximum value (milliseconds)
    /// Maximum delay between retries
    pub backoff_max_ms: u64,
    /// Watch stream restart delay after unknown errors (seconds)
    /// How long to wait before restarting watch stream after an error
    pub watch_restart_delay_secs: u64,
    /// Watch stream restart delay after stream ends (seconds)
    /// How long to wait before restarting watch stream after it ends normally
    pub watch_restart_delay_after_end_secs: u64,
    /// Minimum GitRepository pull interval (seconds)
    /// Enforced minimum to prevent API rate limiting
    pub min_gitrepository_pull_interval_secs: u64,
    /// Minimum reconcile interval (seconds)
    /// Enforced minimum to prevent API rate limiting
    pub min_reconcile_interval_secs: u64,
    /// SOPS private key secret name
    /// Name of the Kubernetes secret containing the SOPS GPG private key
    pub sops_private_key_secret_name: String,
    /// Enable SOPS key watch for hot-reload
    /// When true, watches for changes to SOPS key secret and reloads without restart
    pub sops_key_watch_enabled: bool,
    /// Controller namespace
    /// Namespace where the controller is deployed (for SOPS key lookup)
    pub controller_namespace: String,
    /// Global log level (ERROR, WARN, INFO, DEBUG, TRACE)
    /// Controls overall controller logging (separate from per-resource CRD logging)
    pub log_level: String,
    /// Log format (json, text)
    pub log_format: String,
    /// Enable color in text format logs
    pub log_enable_color: bool,
    /// Enable metrics collection
    pub enable_metrics: bool,
    /// Enable distributed tracing
    pub enable_tracing: bool,
    /// Maximum concurrent reconciliations
    /// Limits how many resources can be reconciled simultaneously
    pub max_concurrent_reconciliations: usize,
    /// Maximum secrets per resource
    /// Prevents resource exhaustion from overly large secret lists
    pub max_secrets_per_resource: usize,
    /// Maximum secret size in bytes
    /// Enforced by validation (64KB default)
    pub max_secret_size_bytes: usize,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        use crate::constants::*;
        Self {
            reconciliation_error_requeue_secs: DEFAULT_RECONCILIATION_ERROR_REQUEUE_SECS,
            backoff_start_ms: DEFAULT_BACKOFF_START_MS,
            backoff_max_ms: DEFAULT_BACKOFF_MAX_MS,
            watch_restart_delay_secs: DEFAULT_WATCH_RESTART_DELAY_SECS,
            watch_restart_delay_after_end_secs: DEFAULT_WATCH_RESTART_DELAY_AFTER_END_SECS,
            min_gitrepository_pull_interval_secs: MIN_GITREPOSITORY_PULL_INTERVAL_SECS,
            min_reconcile_interval_secs: MIN_RECONCILE_INTERVAL_SECS,
            sops_private_key_secret_name: "sops-private-key".to_string(),
            sops_key_watch_enabled: true,
            controller_namespace: "octopilot-system".to_string(),
            log_level: "INFO".to_string(),
            log_format: "json".to_string(),
            log_enable_color: false,
            enable_metrics: true,
            enable_tracing: true,
            max_concurrent_reconciliations: 10,
            max_secrets_per_resource: 1000,
            max_secret_size_bytes: 65536, // 64KB
        }
    }
}

impl ControllerConfig {
    /// Load configuration from environment variables with defaults
    pub fn from_env() -> Self {
        use crate::constants::*;
        Self {
            reconciliation_error_requeue_secs: env_var_or_default(
                "RECONCILIATION_ERROR_REQUEUE_SECS",
                DEFAULT_RECONCILIATION_ERROR_REQUEUE_SECS,
            ),
            backoff_start_ms: env_var_or_default("BACKOFF_START_MS", DEFAULT_BACKOFF_START_MS),
            backoff_max_ms: env_var_or_default("BACKOFF_MAX_MS", DEFAULT_BACKOFF_MAX_MS),
            watch_restart_delay_secs: env_var_or_default(
                "WATCH_RESTART_DELAY_SECS",
                DEFAULT_WATCH_RESTART_DELAY_SECS,
            ),
            watch_restart_delay_after_end_secs: env_var_or_default(
                "WATCH_RESTART_DELAY_AFTER_END_SECS",
                DEFAULT_WATCH_RESTART_DELAY_AFTER_END_SECS,
            ),
            min_gitrepository_pull_interval_secs: env_var_or_default(
                "MIN_GITREPOSITORY_PULL_INTERVAL_SECS",
                MIN_GITREPOSITORY_PULL_INTERVAL_SECS,
            ),
            min_reconcile_interval_secs: env_var_or_default(
                "MIN_RECONCILE_INTERVAL_SECS",
                MIN_RECONCILE_INTERVAL_SECS,
            ),
            sops_private_key_secret_name: env_var_or_default_str(
                "SOPS_PRIVATE_KEY_SECRET_NAME",
                "sops-private-key",
            ),
            sops_key_watch_enabled: env_var_or_default_bool("SOPS_KEY_WATCH_ENABLED", true),
            controller_namespace: env_var_or_default_str("POD_NAMESPACE", "octopilot-system"),
            log_level: env_var_or_default_str("LOG_LEVEL", "INFO"),
            log_format: env_var_or_default_str("LOG_FORMAT", "json"),
            log_enable_color: env_var_or_default_bool("LOG_ENABLE_COLOR", false),
            enable_metrics: env_var_or_default_bool("ENABLE_METRICS", true),
            enable_tracing: env_var_or_default_bool("ENABLE_TRACING", true),
            max_concurrent_reconciliations: env_var_or_default(
                "MAX_CONCURRENT_RECONCILIATIONS",
                10,
            ),
            max_secrets_per_resource: env_var_or_default("MAX_SECRETS_PER_RESOURCE", 1000),
            max_secret_size_bytes: env_var_or_default("MAX_SECRET_SIZE_BYTES", 65536),
        }
    }

    /// Get reconciliation error requeue duration
    pub fn reconciliation_error_requeue_duration(&self) -> Duration {
        Duration::from_secs(self.reconciliation_error_requeue_secs)
    }

    /// Get watch restart delay duration
    pub fn watch_restart_delay_duration(&self) -> Duration {
        Duration::from_secs(self.watch_restart_delay_secs)
    }

    /// Get watch restart delay after end duration
    pub fn watch_restart_delay_after_end_duration(&self) -> Duration {
        Duration::from_secs(self.watch_restart_delay_after_end_secs)
    }

    /// Get backoff start duration
    pub fn backoff_start_duration(&self) -> Duration {
        Duration::from_millis(self.backoff_start_ms)
    }

    /// Get backoff max duration
    pub fn backoff_max_duration(&self) -> Duration {
        Duration::from_millis(self.backoff_max_ms)
    }

    /// Get minimum GitRepository pull interval duration
    pub fn min_gitrepository_pull_interval_duration(&self) -> Duration {
        Duration::from_secs(self.min_gitrepository_pull_interval_secs)
    }

    /// Get minimum reconcile interval duration
    pub fn min_reconcile_interval_duration(&self) -> Duration {
        Duration::from_secs(self.min_reconcile_interval_secs)
    }
}

/// Read environment variable or return default value
fn env_var_or_default<T: std::str::FromStr>(key: &str, default: T) -> T
where
    <T as std::str::FromStr>::Err: std::fmt::Debug,
{
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Read environment variable as boolean or return default
fn env_var_or_default_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            let v_lower = v.to_lowercase();
            v_lower == "true" || v_lower == "1" || v_lower == "yes" || v_lower == "on"
        })
        .unwrap_or(default)
}

/// Read environment variable as string or return default
fn env_var_or_default_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
