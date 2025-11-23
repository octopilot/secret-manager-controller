//! Common utilities for integration tests
//!
//! Provides shared functionality for:
//! - Kind cluster setup and management
//! - Service endpoint discovery
//! - Test environment setup

pub mod cluster_setup;

pub use cluster_setup::{
    ensure_kind_cluster, get_mock_server_endpoint, get_service_endpoint,
    wait_for_controller_ready, wait_for_service_ready,
};

