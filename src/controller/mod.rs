//! # Controller
//!
//! Core controller modules for the Secret Manager Controller.
//!
//! - `backoff`: Fibonacci backoff mechanism for retries
//! - `crdgen`: CRD generation utility
//! - `kustomize`: Kustomize build functionality
//! - `parser`: Configuration file parsing (application.secrets.env, application.properties)
//! - `reconciler`: Core reconciliation logic
//! - `server`: HTTP server for metrics and health checks

pub mod backoff;
pub mod crdgen;
pub mod kustomize;
pub mod parser;
pub mod reconciler;
pub mod server;
