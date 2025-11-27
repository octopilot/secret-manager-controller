//! # Validation Unit Tests
//!
//! Comprehensive unit tests for validation functions.
//!
//! These tests verify:
//! - Kubernetes name/namespace/label validation
//! - Duration parsing and validation
//! - Provider configuration validation
//! - Path and URL validation
//! - Secret name component validation

use controller::controller::reconciler::validation::{
    parse_kubernetes_duration, validate_aws_parameter_path, validate_duration_interval,
    validate_kubernetes_label, validate_kubernetes_name, validate_kubernetes_namespace,
    validate_path, validate_provider_config, validate_secret_name_component,
    validate_source_ref_kind, validate_url,
};
use controller::crd::{AwsConfig, AzureConfig, GcpConfig, ProviderConfig};

#[test]
fn test_validate_kubernetes_name_valid() {
    let max_name = "a".repeat(253);
    let valid_names = vec![
        "my-resource",
        "my-resource-123",
        "my.resource",
        "my.resource.subdomain",
        "a",
        &max_name, // Max length
    ];

    for name in valid_names {
        assert!(
            validate_kubernetes_name(&name, "test").is_ok(),
            "Name '{}' should be valid",
            name
        );
    }
}

#[test]
fn test_validate_kubernetes_name_invalid() {
    let too_long = "a".repeat(254);
    let invalid_names = vec![
        "",             // Empty
        "-invalid",     // Starts with hyphen
        "invalid-",     // Ends with hyphen
        ".invalid",     // Starts with dot
        "invalid.",     // Ends with dot
        "INVALID",      // Uppercase
        "invalid_name", // Underscore
        "invalid name", // Space
        &too_long,      // Too long
        "invalid/name", // Slash
    ];

    for name in invalid_names {
        assert!(
            validate_kubernetes_name(name, "test").is_err(),
            "Name '{}' should be invalid",
            name
        );
    }
}

#[test]
fn test_validate_kubernetes_namespace_valid() {
    let max_namespace = "a".repeat(63);
    let valid_namespaces = vec![
        "default",
        "kube-system",
        "my-namespace",
        "my-namespace-123",
        "a",
        &max_namespace, // Max length
    ];

    for namespace in valid_namespaces {
        assert!(
            validate_kubernetes_namespace(namespace).is_ok(),
            "Namespace '{}' should be valid",
            namespace
        );
    }
}

#[test]
fn test_validate_kubernetes_namespace_invalid() {
    let too_long = "a".repeat(64);
    let invalid_namespaces = vec![
        "",                  // Empty
        "-invalid",          // Starts with hyphen
        "invalid-",          // Ends with hyphen
        "INVALID",           // Uppercase
        "invalid_namespace", // Underscore
        "invalid namespace", // Space
        "invalid.namespace", // Dot
        &too_long,           // Too long
    ];

    for namespace in invalid_namespaces {
        assert!(
            validate_kubernetes_namespace(namespace).is_err(),
            "Namespace '{}' should be invalid",
            namespace
        );
    }
}

#[test]
fn test_validate_kubernetes_label_valid() {
    let max_label = "a".repeat(63);
    let valid_labels = vec![
        "dev",
        "production",
        "my-label",
        "my-label-123",
        "my_label",
        "my.label",
        "a",
        &max_label, // Max length
    ];

    for label in valid_labels {
        assert!(
            validate_kubernetes_label(&label, "test").is_ok(),
            "Label '{}' should be valid",
            label
        );
    }
}

#[test]
fn test_validate_kubernetes_label_invalid() {
    let too_long = "a".repeat(64);
    let invalid_labels = vec![
        "",              // Empty
        ".invalid",      // Starts with dot
        "invalid.",      // Ends with dot
        "INVALID",       // Uppercase
        "invalid label", // Space
        &too_long,       // Too long
    ];

    for label in invalid_labels {
        assert!(
            validate_kubernetes_label(&label, "test").is_err(),
            "Label '{}' should be invalid",
            label
        );
    }
}

#[test]
fn test_validate_source_ref_kind() {
    assert!(validate_source_ref_kind("GitRepository").is_ok());
    assert!(validate_source_ref_kind("Application").is_ok());
    assert!(validate_source_ref_kind("gitrepository").is_err()); // Case sensitive
    assert!(validate_source_ref_kind("GitRepo").is_err());
    assert!(validate_source_ref_kind("").is_err());
}

#[test]
fn test_parse_kubernetes_duration_valid() {
    let test_cases = vec![
        ("1s", 1),
        ("30s", 30),
        ("1m", 60),
        ("5m", 300),
        ("1h", 3600),
        ("2h", 7200),
        ("1d", 86400),
        ("2d", 172800),
    ];

    for (input, expected_seconds) in test_cases {
        let result = parse_kubernetes_duration(input).unwrap();
        assert_eq!(
            result.as_secs(),
            expected_seconds,
            "Duration '{}' should parse to {} seconds",
            input,
            expected_seconds
        );
    }
}

#[test]
fn test_parse_kubernetes_duration_invalid() {
    let invalid_durations = vec![
        "",     // Empty
        "0s",   // Zero
        "1",    // No unit
        "s",    // No number
        "1x",   // Invalid unit
        "1.5m", // Decimal
        "-1m",  // Negative
        "1m30s", // Multiple units
                // Note: parse_kubernetes_duration trims whitespace, so " 1m " is valid
    ];

    for duration in invalid_durations {
        assert!(
            parse_kubernetes_duration(duration).is_err(),
            "Duration '{}' should be invalid",
            duration
        );
    }
}

#[test]
fn test_validate_duration_interval() {
    // Valid durations (meeting minimum)
    assert!(validate_duration_interval("1m", "test", 60).is_ok());
    assert!(validate_duration_interval("5m", "test", 60).is_ok());
    assert!(validate_duration_interval("1h", "test", 60).is_ok());

    // Too short (less than minimum)
    assert!(validate_duration_interval("30s", "test", 60).is_err());
    assert!(validate_duration_interval("1m", "test", 120).is_err());

    // Invalid format
    assert!(validate_duration_interval("", "test", 60).is_err());
    assert!(validate_duration_interval("invalid", "test", 60).is_err());
}

#[test]
fn test_validate_path() {
    // Valid paths
    assert!(validate_path("relative/path", "test").is_ok());
    assert!(validate_path("/absolute/path", "test").is_ok());
    assert!(validate_path("path", "test").is_ok());

    // Invalid paths
    assert!(validate_path("", "test").is_err());
    assert!(validate_path("path\0with\0null", "test").is_err());
}

#[test]
fn test_validate_url() {
    // Valid URLs
    assert!(validate_url("http://example.com", "test").is_ok());
    assert!(validate_url("https://example.com/path", "test").is_ok());
    assert!(validate_url("https://example.com:8080/path?query=value", "test").is_ok());

    // Invalid URLs
    assert!(validate_url("", "test").is_err());
    assert!(validate_url("not-a-url", "test").is_err());
    assert!(validate_url("ftp://example.com", "test").is_err()); // Only http/https
    assert!(validate_url("example.com", "test").is_err()); // No scheme
}

#[test]
fn test_validate_aws_parameter_path() {
    // Valid paths
    assert!(validate_aws_parameter_path("/my-service/dev", "test").is_ok());
    assert!(validate_aws_parameter_path("/my-service/dev/database", "test").is_ok());
    assert!(validate_aws_parameter_path("/service", "test").is_ok());

    // Invalid paths
    assert!(validate_aws_parameter_path("", "test").is_err());
    assert!(validate_aws_parameter_path("my-service/dev", "test").is_err()); // No leading slash
    assert!(validate_aws_parameter_path("/my-service//dev", "test").is_err()); // Double slash
    assert!(validate_aws_parameter_path("/my-service/dev/", "test").is_err()); // Trailing slash
}

#[test]
fn test_validate_secret_name_component() {
    // Valid components
    assert!(validate_secret_name_component("my-secret", "test").is_ok());
    assert!(validate_secret_name_component("my_secret", "test").is_ok());
    assert!(validate_secret_name_component("mySecret123", "test").is_ok());
    assert!(validate_secret_name_component("a", "test").is_ok());

    // Invalid components
    let too_long = "a".repeat(256);
    assert!(validate_secret_name_component("", "test").is_err());
    // Note: validate_secret_name_component trims whitespace, so trailing space is valid after trim
    // Test with non-trimmable invalid characters instead
    assert!(validate_secret_name_component("my-secret/", "test").is_err()); // Slash
    assert!(validate_secret_name_component("my secret", "test").is_err()); // Space in middle
    assert!(validate_secret_name_component(&too_long, "test").is_err()); // Too long
}

#[test]
fn test_validate_provider_config_gcp() {
    // Valid GCP config
    let valid_config = ProviderConfig::Gcp(GcpConfig {
        project_id: "my-project-123".to_string(),
        location: "us-central1".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&valid_config).is_ok());

    // Invalid GCP config - empty project ID
    let invalid_config = ProviderConfig::Gcp(GcpConfig {
        project_id: "".to_string(),
        location: "us-central1".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&invalid_config).is_err());

    // Invalid GCP config - invalid format
    let invalid_config2 = ProviderConfig::Gcp(GcpConfig {
        project_id: "INVALID-PROJECT".to_string(), // Uppercase
        location: "us-central1".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&invalid_config2).is_err());
}

#[test]
fn test_validate_provider_config_aws() {
    // Valid AWS config
    let valid_config = ProviderConfig::Aws(AwsConfig {
        region: "us-east-1".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&valid_config).is_ok());

    // Valid AWS config - gov region
    let valid_config2 = ProviderConfig::Aws(AwsConfig {
        region: "us-gov-west-1".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&valid_config2).is_ok());

    // Invalid AWS config - empty region
    let invalid_config = ProviderConfig::Aws(AwsConfig {
        region: "".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&invalid_config).is_err());

    // Invalid AWS config - invalid format
    let invalid_config2 = ProviderConfig::Aws(AwsConfig {
        region: "invalid-region".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&invalid_config2).is_err());
}

#[test]
fn test_validate_provider_config_azure() {
    // Valid Azure config
    let valid_config = ProviderConfig::Azure(AzureConfig {
        vault_name: "my-vault".to_string(),
        location: "eastus".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&valid_config).is_ok());

    // Invalid Azure config - empty vault name
    let invalid_config = ProviderConfig::Azure(AzureConfig {
        vault_name: "".to_string(),
        location: "eastus".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&invalid_config).is_err());

    // Invalid Azure config - too short
    let invalid_config2 = ProviderConfig::Azure(AzureConfig {
        vault_name: "ab".to_string(), // Too short (min 3)
        location: "eastus".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&invalid_config2).is_err());

    // Invalid Azure config - consecutive hyphens
    let invalid_config3 = ProviderConfig::Azure(AzureConfig {
        vault_name: "my--vault".to_string(),
        location: "eastus".to_string(),
        auth: None,
    });
    assert!(validate_provider_config(&invalid_config3).is_err());
}
