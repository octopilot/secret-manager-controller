//! Test that ProviderConfig can deserialize with or without the `type` field

use controller::crd::provider::ProviderConfig;

#[test]
fn test_provider_config_deserialize_with_type_field() {
    // Test deserialization with type field (as in existing YAML files)
    let json_with_type = r#"{
        "type": "gcp",
        "gcp": {
            "projectId": "test-project",
            "location": "us-central1"
        }
    }"#;
    
    let config: ProviderConfig = serde_json::from_str(json_with_type)
        .expect("Should deserialize with type field");
    
    match config {
        ProviderConfig::Gcp(gcp_config) => {
            assert_eq!(gcp_config.project_id, "test-project");
            assert_eq!(gcp_config.location, "us-central1");
        }
        _ => panic!("Expected Gcp config"),
    }
}

#[test]
fn test_provider_config_deserialize_without_type_field() {
    // Test deserialization without type field (also valid)
    let json_without_type = r#"{
        "gcp": {
            "projectId": "test-project",
            "location": "us-central1"
        }
    }"#;
    
    let config: ProviderConfig = serde_json::from_str(json_without_type)
        .expect("Should deserialize without type field");
    
    match config {
        ProviderConfig::Gcp(gcp_config) => {
            assert_eq!(gcp_config.project_id, "test-project");
            assert_eq!(gcp_config.location, "us-central1");
        }
        _ => panic!("Expected Gcp config"),
    }
}

#[test]
fn test_provider_config_deserialize_aws_with_type() {
    let json = r#"{
        "type": "aws",
        "aws": {
            "region": "us-east-1"
        }
    }"#;
    
    let config: ProviderConfig = serde_json::from_str(json)
        .expect("Should deserialize AWS with type field");
    
    match config {
        ProviderConfig::Aws(aws_config) => {
            assert_eq!(aws_config.region, "us-east-1");
        }
        _ => panic!("Expected Aws config"),
    }
}

#[test]
fn test_provider_config_deserialize_azure_with_type() {
    let json = r#"{
        "type": "azure",
        "azure": {
            "vaultName": "test-vault",
            "location": "eastus"
        }
    }"#;
    
    let config: ProviderConfig = serde_json::from_str(json)
        .expect("Should deserialize Azure with type field");
    
    match config {
        ProviderConfig::Azure(azure_config) => {
            assert_eq!(azure_config.vault_name, "test-vault");
            assert_eq!(azure_config.location, "eastus");
        }
        _ => panic!("Expected Azure config"),
    }
}

