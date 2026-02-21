//! Output format definitions for PathBuilder
//!
//! Different consumers need different path formats:
//! - Routes: Axum route patterns with placeholders
//! - HttpPath: Full HTTP paths with actual values
//! - ResponseName: Response names without /v1/ prefix
//! - PactPath: Paths for Pact contract tests

/// Output format for path construction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathFormat {
    /// Axum route pattern with placeholders: "/v1/projects/{project}/secrets"
    Route,

    /// Full HTTP path with actual values: "/v1/projects/my-project/secrets"
    HttpPath,

    /// Response name without /v1/ prefix: "projects/my-project/secrets/my-secret"
    ResponseName,

    /// Pact contract path: "/v1/projects/test-project/secrets"
    PactPath,
}
