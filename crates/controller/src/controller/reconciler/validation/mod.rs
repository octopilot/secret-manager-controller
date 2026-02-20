//! # Validation
//!
//! Validates SecretManagerConfig resources and duration strings.

mod config;
mod configs;
mod duration;
mod kubernetes;
mod paths;
mod provider;
mod secrets;

pub use config::validate_secret_manager_config;
pub use duration::{parse_kubernetes_duration, validate_duration_interval};
pub use kubernetes::{
    validate_kubernetes_label, validate_kubernetes_name, validate_kubernetes_namespace,
    validate_source_ref_kind,
};
// These validation helpers live in the local paths submodule (not the smc-paths crate)
pub use paths::{validate_aws_parameter_path, validate_path, validate_url};
pub use provider::validate_provider_config;
pub use secrets::validate_secret_name_component;
