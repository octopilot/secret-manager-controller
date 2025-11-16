//! # Constants
//!
//! Shared constants used throughout the controller.
//!
//! These values represent reasonable defaults and can be overridden via
//! configuration or environment variables where applicable.

/// Default HTTP server port for metrics and health probes
pub const DEFAULT_METRICS_PORT: u16 = 5000;

/// Default HTTP server startup timeout (how long to wait for server to be ready)
pub const DEFAULT_SERVER_STARTUP_TIMEOUT_SECS: u64 = 10;

/// Default HTTP server readiness poll interval
pub const DEFAULT_SERVER_POLL_INTERVAL_MS: u64 = 50;

/// Default requeue interval for reconciliation errors (seconds)
pub const DEFAULT_RECONCILIATION_ERROR_REQUEUE_SECS: u64 = 60;

/// Default requeue interval when GitRepository is not found (seconds)
/// Minimum 1 minute to align with GitOps tool conventions and minimum reconcile interval
pub const DEFAULT_GITREPOSITORY_NOT_FOUND_REQUEUE_SECS: u64 = 60;

/// Default exponential backoff starting value (milliseconds)
pub const DEFAULT_BACKOFF_START_MS: u64 = 1000;

/// Default exponential backoff maximum value (milliseconds)
pub const DEFAULT_BACKOFF_MAX_MS: u64 = 30_000;

/// Default delay before restarting watch stream after unknown errors (seconds)
pub const DEFAULT_WATCH_RESTART_DELAY_SECS: u64 = 5;

/// Default delay before restarting watch stream after it ends (seconds)
pub const DEFAULT_WATCH_RESTART_DELAY_AFTER_END_SECS: u64 = 1;

/// Minimum GitRepository pull interval (seconds)
/// Shorter intervals may hit API rate limits
pub const MIN_GITREPOSITORY_PULL_INTERVAL_SECS: u64 = 60;

/// Minimum reconcile interval (seconds)
pub const MIN_RECONCILE_INTERVAL_SECS: u64 = 60;
