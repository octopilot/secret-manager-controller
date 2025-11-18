//! Common test utilities for Pact integration tests
//!
//! Provides shared initialization code for all Pact tests, including
//! rustls crypto provider setup.

use std::sync::Once;

static RUSTLS_INIT: Once = Once::new();

/// Initialize rustls crypto provider for tests
///
/// This must be called before any async operations that use rustls.
/// Uses a `Once` to ensure it's only called once across all tests.
pub fn init_rustls() {
    RUSTLS_INIT.call_once(|| {
        // Configure rustls crypto provider FIRST, before any other operations
        // Required for rustls 0.23+ when no default provider is set via features
        // This must be called synchronously before any async operations that use rustls
        // We use ring as the crypto provider (matches main application)
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("Failed to install rustls crypto provider");
    });
}
