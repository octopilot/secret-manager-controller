//! # SOPS Key Management
//!
//! Handles loading, reloading, and watching SOPS private keys from Kubernetes secrets.
//!
//! ## Module Structure
//!
//! - `load.rs` - Key loading and reloading functions
//! - `watch.rs` - Watch loop for secret changes
//! - `rbac.rs` - RBAC verification

mod load;
mod rbac;
mod store;
mod watch;

// Re-export public API
pub use load::{
    load_sops_private_key, reload_sops_private_key, reload_sops_private_key_from_namespace,
};
pub use store::{SopsKeyStore, controller_namespace};
pub use watch::start_sops_key_watch;
