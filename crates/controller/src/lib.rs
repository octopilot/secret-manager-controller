//! Secret Manager Controller Library
//!
//! This library provides the core functionality for the Secret Manager Controller.
//! Tests are included in the module files (e.g., reconciler.rs).
//!
//! ## Quick Start
//!
//! ```rust
//! use secret_manager_controller::prelude::*;
//! ```
//!
//! This brings commonly used types and traits into scope. For more specific imports,
//! use the individual modules.

// Re-export modules so they can be tested
pub mod config;
pub mod constants;
pub mod controller;
pub mod crd;
pub mod observability;
pub mod prelude;
pub mod provider;
pub mod runtime;
