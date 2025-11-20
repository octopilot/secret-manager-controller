//! # Reconciler
//!
//! Core reconciliation logic for `SecretManagerConfig` resources.
//!
//! The reconciler:
//! - Watches `SecretManagerConfig` resources across all namespaces
//! - Fetches `GitRepository` or `Application` artifacts
//! - Processes application secret files or kustomize builds
//! - Syncs secrets to cloud providers (GCP, AWS, Azure)
//! - Updates resource status with reconciliation results
//!
//! ## Reconciliation Flow
//!
//! 1. Get source (`FluxCD` `GitRepository` or `ArgoCD` `Application`)
//! 2. Extract artifact path
//! 3. Choose mode:
//!    - **Kustomize Build Mode**: Run `kustomize build` and extract secrets
//!    - **Raw File Mode**: Parse `application.secrets.env` files directly
//! 4. Decrypt SOPS-encrypted files if needed
//! 5. Sync secrets to cloud provider
//! 6. Update status

pub mod artifact;
pub mod notifications;
pub mod processing;
pub mod reconcile;
pub mod sops;
pub mod source;
pub mod status;
pub mod types;
pub mod utils;
pub mod validation;

// Re-export public API
pub use reconcile::reconcile;
pub use sops::start_sops_key_watch;
pub use source::start_source_watch;
pub use status::{
    check_sops_key_availability, update_all_resources_in_namespace, update_sops_key_status,
};
pub use types::{BackoffState, Reconciler, ReconcilerError, TriggerSource};

// Re-export utility functions for external use (including tests)
pub use utils::{construct_secret_name, sanitize_secret_name};
