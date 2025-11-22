//! # Prelude
//!
//! Re-exports commonly used types for convenience.
//!
//! This module provides a prelude that can be imported with `use paths::prelude::*;`
//! to bring commonly used types into scope.
//!
//! ## Usage
//!
//! ```rust
//! use paths::prelude::*;
//!
//! let path = PathBuilder::new()
//!     .gcp_operation(GcpOperation::CreateSecret)
//!     .project("my-project")
//!     .build_http_path();
//! ```
//!
//! This brings into scope:
//! - `PathBuilder` - Main builder for constructing paths
//! - `PathBuilderError` - Error type for path building
//! - `PathFormat` - Output format enum
//! - Operation types (`GcpOperation`, `AwsOperation`, `AzureOperation`, `Operation`)
//! - `Provider` - Provider enum

// Core PathBuilder types
pub use crate::builder::PathBuilder;
pub use crate::errors::PathBuilderError;
pub use crate::formats::PathFormat;

// Operation types - most commonly used together with PathBuilder
pub use crate::operations::{AwsOperation, AzureOperation, GcpOperation, Operation};

// Provider enum
pub use crate::provider::Provider;
