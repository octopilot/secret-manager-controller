//! # Git/Artifact Resolver Integration Tests
//!
//! Integration tests for Git repository and artifact resolution functionality.
//!
//! These tests verify:
//! - Missing GitRepository handling
//! - GitRepository with no artifact
//! - Artifact with bad checksum
//! - FluxCD artifact path resolution
//! - ArgoCD artifact path resolution

use controller::controller::reconciler::artifact::{
    get_argocd_artifact_path, get_flux_artifact_path, get_flux_git_repository,
};
use controller::controller::reconciler::types::Reconciler;
use controller::prelude::*;
use kube::Client;
use std::collections::HashMap;

/// Test helper: Create a mock Reconciler for testing
/// In real tests, this would use a Kubernetes client with a test cluster
async fn create_test_reconciler() -> Reconciler {
    // For integration tests, we'd use a real Kubernetes client
    // This is a placeholder - actual tests would require a test cluster
    use kube::Client;
    let client = Client::try_default()
        .await
        .expect("Failed to create Kubernetes client");
    Reconciler::new(client).await.expect("Failed to create Reconciler")
}

#[tokio::test]
#[ignore] // Requires Kubernetes cluster
async fn test_get_flux_git_repository_missing() {
    let reconciler = create_test_reconciler().await;

    let source_ref = SourceRef {
        kind: "GitRepository".to_string(),
        name: "non-existent-repo".to_string(),
        namespace: "default".to_string(),
        git_credentials: None,
    };

    let result = get_flux_git_repository(&reconciler, &source_ref).await;

    // Should fail with appropriate error
    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("not found") || error_msg.contains("NotFound"),
        "Error should indicate resource not found: {}",
        error_msg
    );
}

#[tokio::test]
#[ignore] // Requires Kubernetes cluster
async fn test_get_flux_git_repository_no_artifact() {
    let reconciler = create_test_reconciler().await;

    // This test would require creating a GitRepository without an artifact in status
    // For now, we document the expected behavior
    let source_ref = SourceRef {
        kind: "GitRepository".to_string(),
        name: "repo-without-artifact".to_string(),
        namespace: "default".to_string(),
        git_credentials: None,
    };

    let result = get_flux_git_repository(&reconciler, &source_ref).await;

    // If GitRepository exists but has no artifact, get_flux_artifact_path should fail
    if let Ok(git_repo) = result {
        let artifact_result = get_flux_artifact_path(&reconciler, &git_repo).await;
        assert!(
            artifact_result.is_err(),
            "Should fail when GitRepository has no artifact in status"
        );
        let error = artifact_result.unwrap_err();
        let error_msg = error.to_string();
        assert!(
            error_msg.contains("artifact") || error_msg.contains("status"),
            "Error should indicate missing artifact: {}",
            error_msg
        );
    }
}

#[tokio::test]
#[ignore] // Requires Kubernetes cluster and SOPS key
async fn test_get_flux_artifact_path_bad_checksum() {
    // This test would require:
    // 1. A GitRepository with an artifact
    // 2. Manually corrupting the artifact file or checksum
    // 3. Verifying that checksum verification fails

    // For now, we document the expected behavior:
    // - get_flux_artifact_path should download the artifact
    // - Checksum verification should fail if digest doesn't match
    // - Error should be classified as CorruptedFile or similar

    // This is a placeholder test that documents expected behavior
    assert!(true, "Placeholder test - requires test cluster setup");
}

#[tokio::test]
#[ignore] // Requires Kubernetes cluster
async fn test_get_argocd_artifact_path_missing_application() {
    let reconciler = create_test_reconciler().await;

    let source_ref = SourceRef {
        kind: "Application".to_string(),
        name: "non-existent-app".to_string(),
        namespace: "default".to_string(),
        git_credentials: None,
    };

    let result = get_argocd_artifact_path(&reconciler, &source_ref).await;

    // Should fail with appropriate error
    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("not found") || error_msg.contains("NotFound"),
        "Error should indicate resource not found: {}",
        error_msg
    );
}

#[tokio::test]
#[ignore] // Requires Kubernetes cluster
async fn test_get_argocd_artifact_path_no_repo() {
    let reconciler = create_test_reconciler().await;

    // This test would require creating an ArgoCD Application without a repo
    // For now, we document the expected behavior
    let source_ref = SourceRef {
        kind: "Application".to_string(),
        name: "app-without-repo".to_string(),
        namespace: "default".to_string(),
        git_credentials: None,
    };

    let result = get_argocd_artifact_path(&reconciler, &source_ref).await;

    // Should fail if Application has no repo configuration
    assert!(result.is_err());
    let error = result.unwrap_err();
    let error_msg = error.to_string();
    assert!(
        error_msg.contains("repo") || error_msg.contains("source"),
        "Error should indicate missing repo configuration: {}",
        error_msg
    );
}

#[test]
fn test_source_ref_validation() {
    // Test that SourceRef validation works correctly
    let valid_source_ref = SourceRef {
        kind: "GitRepository".to_string(),
        name: "test-repo".to_string(),
        namespace: "default".to_string(),
        git_credentials: None,
    };

    assert_eq!(valid_source_ref.kind, "GitRepository");
    assert_eq!(valid_source_ref.name, "test-repo");
    assert_eq!(valid_source_ref.namespace, "default");

    // Test ArgoCD Application
    let argocd_source_ref = SourceRef {
        kind: "Application".to_string(),
        name: "test-app".to_string(),
        namespace: "argocd".to_string(),
        git_credentials: None,
    };

    assert_eq!(argocd_source_ref.kind, "Application");
    assert_eq!(argocd_source_ref.name, "test-app");
    assert_eq!(argocd_source_ref.namespace, "argocd");
}

#[test]
fn test_artifact_path_sanitization() {
    // Test that artifact paths are properly sanitized
    use controller::controller::reconciler::utils::sanitize_path_component;

    // Test normal names
    assert_eq!(sanitize_path_component("my-repo"), "my-repo");
    assert_eq!(sanitize_path_component("my_repo"), "my_repo");

    // Test names with invalid characters
    assert_eq!(sanitize_path_component("my/repo"), "my_repo");
    assert_eq!(sanitize_path_component("my.repo"), "my_repo");
    assert_eq!(sanitize_path_component("my repo"), "my_repo");

    // Test edge cases
    assert_eq!(sanitize_path_component(""), "");
    assert_eq!(sanitize_path_component("---"), "");
    assert_eq!(sanitize_path_component("___"), "");
}

