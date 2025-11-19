//! # Secret Processing
//!
//! Handles parsing application files and processing Kustomize builds to extract secrets and properties.

mod application_files;
mod kustomize;
mod properties;
mod secrets;

pub use application_files::process_application_files;
pub use kustomize::process_kustomize_secrets;
