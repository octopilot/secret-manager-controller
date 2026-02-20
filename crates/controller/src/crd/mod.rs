//! # Custom Resource Definitions
//!
//! CRD types for the Secret Manager Controller.
//!
//! This module contains all the Kubernetes Custom Resource Definition types
//! used by the controller, including SecretManagerConfig and its related types.
//!
//! ## Module Structure
//!
//! - `spec.rs` - Main CRD specification and default values
//! - `provider.rs` - Cloud provider configuration (GCP, AWS, Azure)
//! - `source.rs` - Source references and secrets/configs configuration
//! - `status.rs` - Status types for tracking reconciliation state
//! - `otel.rs` - OpenTelemetry configuration

mod hot_reload;
mod logging;
mod notifications;
mod otel;
mod provider;
mod source;
mod spec;
mod status;

// Re-export all public types
pub use hot_reload::HotReloadConfig;
pub use logging::{LogLevel, LoggingConfig};
pub use notifications::{
    ArgoCDNotificationConfig, FluxCDNotificationConfig, NotificationConfig,
    NotificationSubscription, ProviderRef,
};
pub use otel::OtelConfig;
pub use provider::{
    AwsAuthConfig, AwsConfig, AzureAuthConfig, AzureConfig, GcpAuthConfig, GcpConfig,
    ProviderConfig,
};
pub use source::{ConfigStoreType, ConfigsConfig, GitCredentialsRef, SecretsConfig, SourceRef};
pub use spec::{
    SecretManagerConfig, SecretManagerConfigSpec, default_false,
    default_git_repository_pull_interval, default_reconcile_interval, default_source_kind,
    default_true,
};
pub use status::{Condition, ResourceSyncState, SecretManagerConfigStatus, SyncStatus};
