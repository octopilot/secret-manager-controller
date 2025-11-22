//! # Prelude
//!
//! Re-exports commonly used types and traits for convenience.
//!
//! This module provides a prelude that can be imported with `use crate::prelude::*;`
//! to bring commonly used types and traits into scope.
//!
//! ## Usage
//!
//! ```rust
//! use secret_manager_controller::prelude::*;
//! ```
//!
//! This brings into scope:
//! - All CRD types (SecretManagerConfig, ProviderConfig, etc.)
//! - Provider traits (SecretManagerProvider, ConfigStoreProvider)
//! - Reconciler types (Reconciler, ReconcilerError, etc.)
//! - Config types (ControllerConfig, ServerConfig)
//! - Common error types

// CRD types - most commonly used
pub use crate::crd::*;

// Provider traits - needed for implementing providers
pub use crate::provider::{ConfigStoreProvider, SecretManagerProvider};

// Reconciler types - core controller functionality
pub use crate::controller::reconciler::{
    reconcile, BackoffState, Reconciler, ReconcilerError, TriggerSource,
};

// Config types - for configuration management
pub use crate::config::{
    ControllerConfig, ServerConfig, SharedControllerConfig, SharedServerConfig,
};

// Common error types
pub use crate::controller::parser::sops::error::{
    SopsDecryptionError, SopsDecryptionFailureReason,
};

// Re-export commonly used provider implementations for convenience
// Users can still import specific providers if needed
pub use crate::provider::aws::{AwsParameterStore, AwsSecretManager};
pub use crate::provider::azure::{AzureAppConfiguration, AzureKeyVault};
pub use crate::provider::gcp::{ParameterManagerREST, SecretManagerREST};
