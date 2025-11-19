//! # Status Management
//!
//! Updates SecretManagerConfig status with reconciliation results.

mod annotations;
mod backoff;
mod decryption;
mod phase;
mod sops;
mod status;

pub use annotations::{
    clear_manual_trigger_annotation, clear_parsing_error_count, get_parsing_error_count,
    increment_parsing_error_count,
};
pub use backoff::calculate_progressive_backoff;
pub use decryption::update_decryption_status;
pub use phase::update_status_phase;
pub use sops::{
    check_sops_key_availability, update_all_resources_in_namespace, update_sops_key_status,
};
pub use status::update_status;
