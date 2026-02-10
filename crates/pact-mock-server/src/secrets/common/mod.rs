//! Common secret store implementation
//!
//! Provides both in-memory and database-backed secret stores with versioning support.
//! This is shared across all provider-specific implementations.

pub mod db_store;
pub mod entities;
pub mod errors;
pub mod limits;
pub mod store;
pub mod store_enum;
pub mod store_trait;

// Re-export for convenience
pub use db_store::DbSecretStore;
pub use store::{SecretEntry, SecretStore, SecretVersion};
pub use store_enum::SecretStoreEnum;
pub use store_trait::SecretStoreBackend;
