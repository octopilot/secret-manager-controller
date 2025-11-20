//! End-to-End Controller Reconciliation Tests
//!
//! These tests exercise the full controller reconciliation flow:
//! 1. Create SecretManagerConfig with GitRepository reference
//! 2. Create test secret files (application.secrets.env)
//! 3. Trigger reconciliation
//! 4. Verify secrets are created/updated in mock servers
//! 5. Verify controller status updates
//!
//! **Note**: These tests require:
//! - Mock server binaries to be built
//! - A Kubernetes cluster (or use `kube-test` for in-memory testing)
//! - Tests should run sequentially to avoid port conflicts

pub mod aws;
pub mod azure;
pub mod common;
pub mod error_handling;
pub mod gcp;
pub mod gitops_features;
pub mod versioning;
