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
