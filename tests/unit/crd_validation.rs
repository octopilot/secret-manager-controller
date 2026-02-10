//! # CRD Validation Tests
//!
//! Comprehensive tests for all CRD elements to catch schema drift early.
//! These tests validate that all fields can be deserialized correctly and
//! that sample resources match the expected schema.

use controller::crd::{
    AwsAuthConfig, AwsConfig, AzureAuthConfig, AzureConfig, ConfigsConfig, ConfigStoreType,
    GcpAuthConfig, GcpConfig, OtelConfig, ProviderConfig, SecretManagerConfig, SecretsConfig,
    SourceRef,
};

/// Test GCP provider configuration with all fields
#[test]
fn test_gcp_provider_with_auth() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-gcp
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    type: gcp
    gcp:
      projectId: my-gcp-project
      location: us-central1
      auth:
        authType: workloadIdentity
        serviceAccountEmail: sa@project.iam.gserviceaccount.com
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
    basePath: microservices
    prefix: my-service
    suffix: dev
  configs:
    enabled: true
    store: secretManager
  reconcileInterval: "1m"
  gitRepositoryPullInterval: "5m"
  diffDiscovery: true
  triggerUpdate: true
  suspend: false
  suspendGitPulls: false
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize GCP config with all fields");

    // Validate provider
    match &config.spec.provider {
        ProviderConfig::Gcp(gcp) => {
            assert_eq!(gcp.project_id, "my-gcp-project");
            assert_eq!(gcp.location, "us-central1");
            assert!(gcp.auth.is_some());
            match gcp.auth.as_ref().unwrap() {
                GcpAuthConfig::WorkloadIdentity { service_account_email } => {
                    assert_eq!(service_account_email, "sa@project.iam.gserviceaccount.com");
                }
            }
        }
        _ => panic!("Expected GCP provider"),
    }

    // Validate secrets config
    assert_eq!(config.spec.secrets.environment, "dev");
    assert_eq!(
        config.spec.secrets.kustomize_path,
        Some("microservices/my-service/deployment-configuration/profiles/dev".to_string())
    );
    assert_eq!(config.spec.secrets.base_path, Some("microservices".to_string()));
    assert_eq!(config.spec.secrets.prefix, Some("my-service".to_string()));
    assert_eq!(config.spec.secrets.suffix, Some("dev".to_string()));

    // Validate configs
    let configs = config.spec.configs.as_ref().unwrap();
    assert!(configs.enabled);
    assert_eq!(configs.store, Some(ConfigStoreType::SecretManager));

    // Validate intervals
    assert_eq!(config.spec.reconcile_interval, "1m");
    assert_eq!(config.spec.git_repository_pull_interval, "5m");

    // Validate flags
    assert!(config.spec.diff_discovery);
    assert!(config.spec.trigger_update);
    assert!(!config.spec.suspend);
    assert!(!config.spec.suspend_git_pulls);
}

/// Test GCP provider without type field (also valid)
#[test]
fn test_gcp_provider_without_type_field() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-gcp
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize GCP config without type field");

    match &config.spec.provider {
        ProviderConfig::Gcp(gcp) => {
            assert_eq!(gcp.project_id, "my-gcp-project");
            assert_eq!(gcp.location, "us-central1");
        }
        _ => panic!("Expected GCP provider"),
    }
}

/// Test GCP provider without auth (defaults to Workload Identity)
#[test]
fn test_gcp_provider_without_auth() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-gcp
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize GCP config without auth");

    match &config.spec.provider {
        ProviderConfig::Gcp(gcp) => {
            assert_eq!(gcp.project_id, "my-gcp-project");
            // Auth is optional and defaults to None (controller will use Workload Identity)
            assert!(gcp.auth.is_none());
        }
        _ => panic!("Expected GCP provider"),
    }
}

/// Test AWS provider configuration
#[test]
fn test_aws_provider_with_auth() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-aws
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    type: aws
    aws:
      region: us-east-1
      auth:
        authType: irsa
        roleArn: arn:aws:iam::123456789012:role/my-role
  secrets:
    environment: dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize AWS config with auth");

    match &config.spec.provider {
        ProviderConfig::Aws(aws) => {
            assert_eq!(aws.region, "us-east-1");
            assert!(aws.auth.is_some());
            match aws.auth.as_ref().unwrap() {
                AwsAuthConfig::Irsa { role_arn } => {
                    assert_eq!(role_arn, "arn:aws:iam::123456789012:role/my-role");
                }
            }
        }
        _ => panic!("Expected AWS provider"),
    }
}

/// Test AWS provider with configs enabled
#[test]
fn test_aws_provider_with_configs() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-aws
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    aws:
      region: us-east-1
  secrets:
    environment: dev
  configs:
    enabled: true
    parameterPath: /my-service/dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize AWS config with configs");

    let configs = config.spec.configs.as_ref().unwrap();
    assert!(configs.enabled);
    assert_eq!(configs.parameter_path, Some("/my-service/dev".to_string()));
}

/// Test Azure provider configuration
#[test]
fn test_azure_provider_with_auth() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-azure
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    type: azure
    azure:
      vaultName: my-vault
      location: eastus
      auth:
        authType: workloadIdentity
        clientId: 12345678-1234-1234-1234-123456789012
  secrets:
    environment: dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize Azure config with auth");

    match &config.spec.provider {
        ProviderConfig::Azure(azure) => {
            assert_eq!(azure.vault_name, "my-vault");
            assert!(azure.auth.is_some());
            match azure.auth.as_ref().unwrap() {
                AzureAuthConfig::WorkloadIdentity { client_id } => {
                    assert_eq!(client_id, "12345678-1234-1234-1234-123456789012");
                }
            }
        }
        _ => panic!("Expected Azure provider"),
    }
}

/// Test Azure provider with configs enabled
#[test]
fn test_azure_provider_with_configs() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-azure
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    azure:
      vaultName: my-vault
      location: eastus
  secrets:
    environment: dev
  configs:
    enabled: true
    appConfigEndpoint: https://my-app-config.azconfig.io
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize Azure config with configs");

    let configs = config.spec.configs.as_ref().unwrap();
    assert!(configs.enabled);
    assert_eq!(
        configs.app_config_endpoint,
        Some("https://my-app-config.azconfig.io".to_string())
    );
}

/// Test ArgoCD Application source reference
#[test]
fn test_argocd_application_source() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-argocd
  namespace: default
spec:
  sourceRef:
    kind: Application
    name: my-app
    namespace: argocd
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize ArgoCD Application source");

    assert_eq!(config.spec.source_ref.kind, "Application");
    assert_eq!(config.spec.source_ref.name, "my-app");
    assert_eq!(config.spec.source_ref.namespace, "argocd");
}

/// Test minimal configuration (only required fields)
#[test]
fn test_minimal_configuration() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-minimal
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize minimal config");

    // SourceRef kind defaults to "GitRepository"
    assert_eq!(config.spec.source_ref.kind, "GitRepository");
    assert_eq!(config.spec.source_ref.name, "my-repo");
    assert_eq!(config.spec.source_ref.namespace, "octopilot-system");

    // Secrets config minimal
    assert_eq!(config.spec.secrets.environment, "dev");
    assert!(config.spec.secrets.kustomize_path.is_none());
    assert!(config.spec.secrets.base_path.is_none());
    assert!(config.spec.secrets.prefix.is_none());
    assert!(config.spec.secrets.suffix.is_none());

    // Optional fields should be None or defaults
    assert!(config.spec.configs.is_none());
    assert!(config.spec.otel.is_none());

    // Default intervals
    assert_eq!(config.spec.reconcile_interval, "1m");
    assert_eq!(config.spec.git_repository_pull_interval, "5m");

    // Default flags
    assert!(config.spec.diff_discovery);
    assert!(config.spec.trigger_update);
    assert!(!config.spec.suspend);
    assert!(!config.spec.suspend_git_pulls);
}

/// Test OpenTelemetry OTLP configuration
#[test]
fn test_otel_otlp_configuration() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-otel-otlp
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
  otel:
    type: otlp
    endpoint: http://otel-collector:4317
    serviceName: secret-manager-controller
    serviceVersion: 1.0.0
    environment: production
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize OTLP config");

    let otel = config.spec.otel.as_ref().unwrap();
    match otel {
        OtelConfig::Otlp {
            endpoint,
            service_name,
            service_version,
            environment,
        } => {
            assert_eq!(endpoint, &Some("http://otel-collector:4317".to_string()));
            assert_eq!(service_name, &Some("secret-manager-controller".to_string()));
            assert_eq!(service_version, &Some("1.0.0".to_string()));
            assert_eq!(environment, &Some("production".to_string()));
        }
        _ => panic!("Expected OTLP config"),
    }
}

/// Test OpenTelemetry Datadog configuration
#[test]
fn test_otel_datadog_configuration() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-otel-datadog
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
  otel:
    type: datadog
    site: datadoghq.com
    apiKey: my-api-key
    serviceName: secret-manager-controller
    serviceVersion: 1.0.0
    environment: production
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize Datadog config");

    let otel = config.spec.otel.as_ref().unwrap();
    match otel {
        OtelConfig::Datadog {
            site,
            api_key,
            service_name,
            service_version,
            environment,
        } => {
            assert_eq!(site, &Some("datadoghq.com".to_string()));
            assert_eq!(api_key, &Some("my-api-key".to_string()));
            assert_eq!(service_name, &Some("secret-manager-controller".to_string()));
            assert_eq!(service_version, &Some("1.0.0".to_string()));
            assert_eq!(environment, &Some("production".to_string()));
        }
        _ => panic!("Expected Datadog config"),
    }
}

/// Test GCP Parameter Manager config store type
#[test]
fn test_gcp_parameter_manager_store() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-gcp-param-manager
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
  configs:
    enabled: true
    store: ParameterManager
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize Parameter Manager store type");

    let configs = config.spec.configs.as_ref().unwrap();
    assert!(configs.enabled);
    assert_eq!(configs.store, Some(ConfigStoreType::ParameterManager));
}

/// Test secrets config with all optional fields
#[test]
fn test_secrets_config_all_fields() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-secrets-all
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
    kustomizePath: microservices/my-service/deployment-configuration/profiles/dev
    basePath: microservices
    prefix: my-service
    suffix: dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize secrets config with all fields");

    assert_eq!(config.spec.secrets.environment, "dev");
    assert_eq!(
        config.spec.secrets.kustomize_path,
        Some("microservices/my-service/deployment-configuration/profiles/dev".to_string())
    );
    assert_eq!(config.spec.secrets.base_path, Some("microservices".to_string()));
    assert_eq!(config.spec.secrets.prefix, Some("my-service".to_string()));
    assert_eq!(config.spec.secrets.suffix, Some("dev".to_string()));
}

/// Test suspended configuration
#[test]
fn test_suspended_configuration() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-suspended
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
  suspend: true
  suspendGitPulls: true
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize suspended config");

    assert!(config.spec.suspend);
    assert!(config.spec.suspend_git_pulls);
}

/// Test all three providers with type field (compatibility test)
#[test]
fn test_all_providers_with_type_field() {
    // GCP
    let gcp_yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-gcp
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    type: gcp
    gcp:
      projectId: my-gcp-project
      location: us-central1
  secrets:
    environment: dev
"#;

    let gcp_config: SecretManagerConfig = serde_yaml::from_str(gcp_yaml)
        .expect("Should deserialize GCP with type field");
    assert!(matches!(gcp_config.spec.provider, ProviderConfig::Gcp(_)));

    // AWS
    let aws_yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-aws
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    type: aws
    aws:
      region: us-east-1
  secrets:
    environment: dev
"#;

    let aws_config: SecretManagerConfig = serde_yaml::from_str(aws_yaml)
        .expect("Should deserialize AWS with type field");
    assert!(matches!(aws_config.spec.provider, ProviderConfig::Aws(_)));

    // Azure
    let azure_yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-azure
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    type: azure
    azure:
      vaultName: my-vault
      location: eastus
  secrets:
    environment: dev
"#;

    let azure_config: SecretManagerConfig = serde_yaml::from_str(azure_yaml)
        .expect("Should deserialize Azure with type field");
    assert!(matches!(azure_config.spec.provider, ProviderConfig::Azure(_)));
}

/// Test configs disabled (default behavior)
#[test]
fn test_configs_disabled() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-configs-disabled
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
  configs:
    enabled: false
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize configs disabled");

    let configs = config.spec.configs.as_ref().unwrap();
    assert!(!configs.enabled);
}

/// Test different interval formats
#[test]
fn test_interval_formats() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-intervals
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
  reconcileInterval: "30s"
  gitRepositoryPullInterval: "10m"
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize different interval formats");

    assert_eq!(config.spec.reconcile_interval, "30s");
    assert_eq!(config.spec.git_repository_pull_interval, "10m");
}

/// Test FluxCD notifications configuration
#[test]
fn test_fluxcd_notifications() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-fluxcd-notifications
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
  notifications:
    fluxcd:
      providerRef:
        name: slack-notifications
        namespace: flux-system
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize FluxCD notifications config");

    let notifications = config.spec.notifications.as_ref().unwrap();
    assert!(notifications.fluxcd.is_some());
    let fluxcd = notifications.fluxcd.as_ref().unwrap();
    assert_eq!(fluxcd.provider_ref.name, "slack-notifications");
    assert_eq!(
        fluxcd.provider_ref.namespace,
        Some("flux-system".to_string())
    );
    // ArgoCD should be None
    assert!(notifications.argocd.is_none());
}

/// Test FluxCD notifications without namespace (defaults to config namespace)
#[test]
fn test_fluxcd_notifications_without_namespace() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-fluxcd-notifications
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
  notifications:
    fluxcd:
      providerRef:
        name: slack-notifications
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize FluxCD notifications without namespace");

    let notifications = config.spec.notifications.as_ref().unwrap();
    let fluxcd = notifications.fluxcd.as_ref().unwrap();
    assert_eq!(fluxcd.provider_ref.name, "slack-notifications");
    // Namespace should be None (will default to config namespace in controller)
    assert!(fluxcd.provider_ref.namespace.is_none());
}

/// Test ArgoCD notifications configuration
#[test]
fn test_argocd_notifications() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-argocd-notifications
  namespace: default
spec:
  sourceRef:
    kind: Application
    name: my-app
    namespace: argocd
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
  notifications:
    argocd:
      subscriptions:
        - trigger: drift-detected
          service: slack
          channel: "#secrets-alerts"
        - trigger: drift-detected
          service: email
          channel: "team@example.com"
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize ArgoCD notifications config");

    let notifications = config.spec.notifications.as_ref().unwrap();
    assert!(notifications.argocd.is_some());
    let argocd = notifications.argocd.as_ref().unwrap();
    assert_eq!(argocd.subscriptions.len(), 2);
    assert_eq!(argocd.subscriptions[0].trigger, "drift-detected");
    assert_eq!(argocd.subscriptions[0].service, "slack");
    assert_eq!(argocd.subscriptions[0].channel, "#secrets-alerts");
    assert_eq!(argocd.subscriptions[1].trigger, "drift-detected");
    assert_eq!(argocd.subscriptions[1].service, "email");
    assert_eq!(argocd.subscriptions[1].channel, "team@example.com");
    // FluxCD should be None
    assert!(notifications.fluxcd.is_none());
}

/// Test both FluxCD and ArgoCD notifications configured (for mixed environments)
#[test]
fn test_both_notifications_configured() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-both-notifications
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
  notifications:
    fluxcd:
      providerRef:
        name: slack-notifications
        namespace: flux-system
    argocd:
      subscriptions:
        - trigger: drift-detected
          service: slack
          channel: "#secrets-alerts"
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize both notifications config");

    let notifications = config.spec.notifications.as_ref().unwrap();
    // Both should be configured
    assert!(notifications.fluxcd.is_some());
    assert!(notifications.argocd.is_some());
    
    let fluxcd = notifications.fluxcd.as_ref().unwrap();
    assert_eq!(fluxcd.provider_ref.name, "slack-notifications");
    
    let argocd = notifications.argocd.as_ref().unwrap();
    assert_eq!(argocd.subscriptions.len(), 1);
    assert_eq!(argocd.subscriptions[0].trigger, "drift-detected");
}

/// Test notifications field is optional (can be omitted)
#[test]
fn test_notifications_optional() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-no-notifications
  namespace: default
spec:
  sourceRef:
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize config without notifications");

    // Notifications should be None (optional field)
    assert!(config.spec.notifications.is_none());
}

/// Test FluxCD notifications with GitRepository source (correct match)
#[test]
fn test_fluxcd_notifications_with_gitrepository() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-fluxcd-gitrepo
  namespace: default
spec:
  sourceRef:
    kind: GitRepository
    name: my-repo
    namespace: octopilot-system
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
  notifications:
    fluxcd:
      providerRef:
        name: slack-notifications
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize FluxCD notifications with GitRepository");

    assert_eq!(config.spec.source_ref.kind, "GitRepository");
    assert!(config.spec.notifications.is_some());
    assert!(config.spec.notifications.as_ref().unwrap().fluxcd.is_some());
}

/// Test ArgoCD notifications with Application source (correct match)
#[test]
fn test_argocd_notifications_with_application() {
    let yaml = r#"
apiVersion: secret-management.octopilot.io/v1beta1
kind: SecretManagerConfig
metadata:
  name: test-argocd-app
  namespace: default
spec:
  sourceRef:
    kind: Application
    name: my-app
    namespace: argocd
  provider:
    gcp:
      projectId: my-gcp-project
  secrets:
    environment: dev
  notifications:
    argocd:
      subscriptions:
        - trigger: drift-detected
          service: webhook
          channel: "https://example.com/webhook"
"#;

    let config: SecretManagerConfig = serde_yaml::from_str(yaml)
        .expect("Should deserialize ArgoCD notifications with Application");

    assert_eq!(config.spec.source_ref.kind, "Application");
    assert!(config.spec.notifications.is_some());
    let notifications = config.spec.notifications.as_ref().unwrap();
    assert!(notifications.argocd.is_some());
    let argocd = notifications.argocd.as_ref().unwrap();
    assert_eq!(argocd.subscriptions[0].service, "webhook");
    assert_eq!(argocd.subscriptions[0].channel, "https://example.com/webhook");
}

