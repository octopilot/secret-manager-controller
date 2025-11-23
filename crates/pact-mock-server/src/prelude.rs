//! # Prelude
//!
//! Re-exports commonly used types and functions for convenience.
//!
//! This module provides a prelude that can be imported with `use pact_mock_server::prelude::*;`
//! to bring commonly used types and functions into scope.
//!
//! ## Usage
//!
//! ```rust
//! use pact_mock_server::prelude::*;
//!
//! // Now you have access to:
//! // - AppState
//! // - Middleware functions
//! // - Secret store types
//! // - Common utilities
//! ```
//!
//! This brings into scope:
//! - `AppState` - Application state for mock servers
//! - Middleware functions (logging, rate limiting, auth, etc.)
//! - Secret store types (`AwsSecretStore`, `GcpSecretStore`, `AzureSecretStore`, etc.)
//! - Common utilities (error responses, validation functions)

// Application state
pub use crate::AppState;

// Middleware functions - commonly used together
pub use crate::{
    auth_failure_middleware, health_check, load_contracts_from_broker, logging_middleware,
    rate_limit_middleware, service_unavailable_middleware, wait_for_broker_and_pacts,
    wait_for_manager_ready,
};

// Secret store types - provider-specific implementations
pub use crate::secrets::aws::AwsSecretStore;
pub use crate::secrets::azure::AzureSecretStore;
pub use crate::secrets::common::{SecretEntry, SecretStore, SecretVersion};
pub use crate::secrets::gcp::{GcpParameterStore, GcpSecretStore};

// Common utilities - error responses and validation
pub use crate::secrets::common::errors::{
    aws_error_response, aws_error_type_from_status, azure_error_code_from_status,
    azure_error_response, gcp_error_response,
};
pub use crate::secrets::common::limits::{
    validate_aws_secret_size, validate_azure_secret_size, validate_gcp_secret_size,
};
