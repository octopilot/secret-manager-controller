//! Shared API path definitions for GCP, AWS, and Azure
//!
//! This crate centralizes all API paths to ensure consistency
//! between the controller and mock server implementations for all providers.
//!
//! ## Quick Start
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
//! ## PathBuilder
//!
//! The `PathBuilder` provides a type-safe, builder-pattern API for constructing
//! API paths with different output formats (routes, HTTP paths, response names, etc.).
//!
//! ## Route Constants
//!
//! Route constants are provided for Axum routes, which require static string literals.
//! These constants are validated against PathBuilder output in tests.

pub mod aws;
pub mod azure;
pub mod gcp;

// Core PathBuilder components
pub mod builder;
pub mod errors;
pub mod formats;
pub mod operations;
pub mod prelude;
pub mod provider;
