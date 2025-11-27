//! # Provider Location/Region Required Tests
//!
//! These tests enforce that location/region is required for all providers.
//! This ensures that Kubernetes will reject SecretManagerConfig resources
//! that don't include the required location/region field.

use controller::crd::ProviderConfig;

/// Test that GCP config without location fails deserialization
#[test]
fn test_gcp_provider_missing_location_fails() {
    let json = r#"{
        "gcp": {
            "projectId": "test-project"
        }
    }"#;

    let result: Result<ProviderConfig, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "GCP config without location should fail deserialization"
    );
    
    // Verify the error message mentions location
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("location") || error_msg.contains("missing field"),
        "Error should mention missing location field. Got: {}",
        error_msg
    );
}

/// Test that GCP config with location succeeds
#[test]
fn test_gcp_provider_with_location_succeeds() {
    let json = r#"{
        "gcp": {
            "projectId": "test-project",
            "location": "us-central1"
        }
    }"#;

    let config: ProviderConfig = serde_json::from_str(json)
        .expect("GCP config with location should deserialize successfully");

    match config {
        ProviderConfig::Gcp(gcp_config) => {
            assert_eq!(gcp_config.project_id, "test-project");
            assert_eq!(gcp_config.location, "us-central1");
        }
        _ => panic!("Expected GCP provider"),
    }
}

/// Test that GCP config with empty location string still deserializes (but should be validated elsewhere)
#[test]
fn test_gcp_provider_with_empty_location_deserializes() {
    let json = r#"{
        "gcp": {
            "projectId": "test-project",
            "location": ""
        }
    }"#;

    let config: ProviderConfig = serde_json::from_str(json)
        .expect("GCP config with empty location should deserialize (validation happens at CRD level)");

    match config {
        ProviderConfig::Gcp(gcp_config) => {
            assert_eq!(gcp_config.project_id, "test-project");
            assert_eq!(gcp_config.location, "");
        }
        _ => panic!("Expected GCP provider"),
    }
}

/// Test that AWS config without region fails deserialization
#[test]
fn test_aws_provider_missing_region_fails() {
    let json = r#"{
        "aws": {}
    }"#;

    let result: Result<ProviderConfig, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "AWS config without region should fail deserialization"
    );
    
    // Verify the error message mentions region
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("region") || error_msg.contains("missing field"),
        "Error should mention missing region field. Got: {}",
        error_msg
    );
}

/// Test that AWS config with region succeeds
#[test]
fn test_aws_provider_with_region_succeeds() {
    let json = r#"{
        "aws": {
            "region": "us-east-1"
        }
    }"#;

    let config: ProviderConfig = serde_json::from_str(json)
        .expect("AWS config with region should deserialize successfully");

    match config {
        ProviderConfig::Aws(aws_config) => {
            assert_eq!(aws_config.region, "us-east-1");
        }
        _ => panic!("Expected AWS provider"),
    }
}

/// Test that Azure config without location fails deserialization
#[test]
fn test_azure_provider_missing_location_fails() {
    let json = r#"{
        "azure": {
            "vaultName": "test-vault"
        }
    }"#;

    let result: Result<ProviderConfig, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Azure config without location should fail deserialization"
    );
    
    // Verify the error message mentions location
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("location") || error_msg.contains("missing field"),
        "Error should mention missing location field. Got: {}",
        error_msg
    );
}

/// Test that Azure config with location succeeds
#[test]
fn test_azure_provider_with_location_succeeds() {
    let json = r#"{
        "azure": {
            "vaultName": "test-vault",
            "location": "eastus"
        }
    }"#;

    let config: ProviderConfig = serde_json::from_str(json)
        .expect("Azure config with location should deserialize successfully");

    match config {
        ProviderConfig::Azure(azure_config) => {
            assert_eq!(azure_config.vault_name, "test-vault");
            assert_eq!(azure_config.location, "eastus");
        }
        _ => panic!("Expected Azure provider"),
    }
}

/// Test that Azure config without vaultName also fails (both are required)
#[test]
fn test_azure_provider_missing_vault_name_fails() {
    let json = r#"{
        "azure": {
            "location": "eastus"
        }
    }"#;

    let result: Result<ProviderConfig, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Azure config without vaultName should fail deserialization"
    );
    
    // Verify the error message mentions vaultName
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("vaultName") || error_msg.contains("vault_name") || error_msg.contains("missing field"),
        "Error should mention missing vaultName field. Got: {}",
        error_msg
    );
}

/// Test all three providers with valid location/region
#[test]
fn test_all_providers_with_valid_location() {
    // GCP
    let gcp_json = r#"{
        "gcp": {
            "projectId": "test-project",
            "location": "us-central1"
        }
    }"#;

    let gcp_config: ProviderConfig = serde_json::from_str(gcp_json)
        .expect("GCP config should deserialize");
    match gcp_config {
        ProviderConfig::Gcp(gcp) => {
            assert_eq!(gcp.location, "us-central1");
        }
        _ => panic!("Expected GCP"),
    }

    // AWS
    let aws_json = r#"{
        "aws": {
            "region": "us-east-1"
        }
    }"#;

    let aws_config: ProviderConfig = serde_json::from_str(aws_json)
        .expect("AWS config should deserialize");
    match aws_config {
        ProviderConfig::Aws(aws) => {
            assert_eq!(aws.region, "us-east-1");
        }
        _ => panic!("Expected AWS"),
    }

    // Azure
    let azure_json = r#"{
        "azure": {
            "vaultName": "test-vault",
            "location": "eastus"
        }
    }"#;

    let azure_config: ProviderConfig = serde_json::from_str(azure_json)
        .expect("Azure config should deserialize");
    match azure_config {
        ProviderConfig::Azure(azure) => {
            assert_eq!(azure.location, "eastus");
        }
        _ => panic!("Expected Azure"),
    }
}

