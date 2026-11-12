//! # Provider Configuration
//!
//! Provider configuration types for GCP, AWS, Azure, and OpenBao.

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Cloud provider configuration
/// Supports GCP, AWS, and Azure Secret Manager
/// Kubernetes sends data in format: {"type": "gcp", "gcp": {...}}
/// We use externally tagged format and ignore the "type" field during deserialization
/// The "type" field is allowed in the schema for compatibility but is ignored during deserialization
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderConfig {
    /// Google Cloud Platform Secret Manager
    #[serde(rename = "gcp")]
    Gcp(GcpConfig),
    /// Amazon Web Services Secrets Manager
    #[serde(rename = "aws")]
    Aws(AwsConfig),
    /// Microsoft Azure Key Vault
    #[serde(rename = "azure")]
    Azure(AzureConfig),
    /// In-cluster OpenBao (KV v2)
    #[serde(rename = "openbao")]
    OpenBao(OpenBaoConfig),
}

impl JsonSchema for ProviderConfig {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("ProviderConfig")
    }

    fn json_schema(schema_gen: &mut SchemaGenerator) -> Schema {
        // Generate schemas for nested configs using the generator
        let gcp_schema = GcpConfig::json_schema(schema_gen);
        let aws_schema = AwsConfig::json_schema(schema_gen);
        let azure_schema = AzureConfig::json_schema(schema_gen);
        let openbao_schema = OpenBaoConfig::json_schema(schema_gen);

        // Convert schemas to JSON values for inclusion in the parent schema
        let mut gcp_json =
            serde_json::to_value(&gcp_schema).unwrap_or_else(|_| serde_json::json!({}));
        let mut aws_json =
            serde_json::to_value(&aws_schema).unwrap_or_else(|_| serde_json::json!({}));
        let mut azure_json =
            serde_json::to_value(&azure_schema).unwrap_or_else(|_| serde_json::json!({}));
        let openbao_json =
            serde_json::to_value(&openbao_schema).unwrap_or_else(|_| serde_json::json!({}));

        // Manually add pattern validation to location/region fields
        // This is done by modifying the schema JSON after generation
        // GCP location pattern: [continent]-[direction][number] (e.g., us-central1)
        if let Some(props) = gcp_json
            .get_mut("properties")
            .and_then(|p| p.as_object_mut())
        {
            if let Some(location) = props.get_mut("location").and_then(|l| l.as_object_mut()) {
                location.insert(
                    "pattern".to_string(),
                    serde_json::json!("^[a-z]+-[a-z]+[0-9]+$"),
                );
            }
        }

        // AWS region pattern: supports standard, gov, iso, china, and local formats
        if let Some(props) = aws_json
            .get_mut("properties")
            .and_then(|p| p.as_object_mut())
        {
            if let Some(region) = props.get_mut("region").and_then(|r| r.as_object_mut()) {
                region.insert("pattern".to_string(), serde_json::json!("^[a-z]{2}-[a-z]+-[0-9]+$|^[a-z]{2}-gov-[a-z]+-[0-9]+$|^[a-z]{2}-iso-[a-z]+-[0-9]+$|^cn-[a-z]+-[0-9]+$|^local$"));
            }
        }

        // Azure location pattern: [direction][region][number] (e.g., eastus, westus2)
        if let Some(props) = azure_json
            .get_mut("properties")
            .and_then(|p| p.as_object_mut())
        {
            if let Some(location) = props.get_mut("location").and_then(|l| l.as_object_mut()) {
                location.insert("pattern".to_string(), serde_json::json!("^[a-z]+[0-9]*$"));
            }
        }

        // Create schema that allows "type" field for compatibility
        // The "type" field is ignored during deserialization but allowed in YAML
        let schema_value = serde_json::json!({
            "type": "object",
            "description": "Cloud provider configuration - supports GCP, AWS, and Azure Secret Manager",
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["gcp", "aws", "azure", "openbao"],
                    "description": "Provider type (optional, ignored during deserialization - use gcp/aws/azure/openbao fields instead)"
                },
                "gcp": gcp_json,
                "aws": aws_json,
                "azure": azure_json,
                "openbao": openbao_json
            },
            "oneOf": [
                {"required": ["gcp"]},
                {"required": ["aws"]},
                {"required": ["azure"]},
                {"required": ["openbao"]}
            ]
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for ProviderConfig")
    }
}

impl<'de> serde::Deserialize<'de> for ProviderConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct ProviderConfigVisitor;

        impl<'de> Visitor<'de> for ProviderConfigVisitor {
            type Value = ProviderConfig;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a provider config object with gcp, aws, azure, or openbao field")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut gcp: Option<GcpConfig> = None;
                let mut aws: Option<AwsConfig> = None;
                let mut azure: Option<AzureConfig> = None;
                let mut openbao: Option<OpenBaoConfig> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "gcp" => {
                            if gcp.is_some() {
                                return Err(de::Error::duplicate_field("gcp"));
                            }
                            // Deserialize GcpConfig from the nested object
                            // The JSON has {"projectId": "..."} which should map to project_id via rename_all
                            gcp = Some(map.next_value::<GcpConfig>().map_err(|e| {
                                de::Error::custom(format!("Failed to deserialize GcpConfig: {e}"))
                            })?);
                        }
                        "aws" => {
                            if aws.is_some() {
                                return Err(de::Error::duplicate_field("aws"));
                            }
                            aws = Some(map.next_value()?);
                        }
                        "azure" => {
                            if azure.is_some() {
                                return Err(de::Error::duplicate_field("azure"));
                            }
                            azure = Some(map.next_value()?);
                        }
                        "openbao" => {
                            if openbao.is_some() {
                                return Err(de::Error::duplicate_field("openbao"));
                            }
                            openbao = Some(map.next_value()?);
                        }
                        "type" => {
                            // Ignore the "type" field - it's redundant
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                        _ => {
                            // Ignore unknown fields (like "type")
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                match (gcp, aws, azure, openbao) {
                    (Some(config), None, None, None) => Ok(ProviderConfig::Gcp(config)),
                    (None, Some(config), None, None) => Ok(ProviderConfig::Aws(config)),
                    (None, None, Some(config), None) => Ok(ProviderConfig::Azure(config)),
                    (None, None, None, Some(config)) => Ok(ProviderConfig::OpenBao(config)),
                    (None, None, None, None) => {
                        Err(de::Error::missing_field("gcp, aws, azure, or openbao"))
                    }
                    _ => Err(de::Error::custom("multiple provider types specified")),
                }
            }
        }

        deserializer.deserialize_map(ProviderConfigVisitor)
    }
}

/// GCP configuration for Secret Manager
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpConfig {
    /// GCP project ID for Secret Manager
    pub project_id: String,
    /// GCP location/region for Secret Manager (e.g., "us-central1", "europe-west1")
    /// Required: Must be specified for all GCP configurations
    /// Format: [continent]-[direction][number] (e.g., us-central1, europe-west1)
    /// See: https://cloud.google.com/about/locations
    pub location: String,
    /// GCP authentication configuration. If not specified, defaults to Workload Identity (recommended).
    #[serde(default)]
    pub auth: Option<GcpAuthConfig>,
}

/// AWS configuration for Secrets Manager
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AwsConfig {
    /// AWS region for Secrets Manager (e.g., "us-east-1", "eu-west-1", "us-gov-west-1", "cn-north-1")
    /// Format: [a-z]{2}-[a-z]+-[0-9]+ (standard) or [a-z]{2}-gov-[a-z]+-[0-9]+ (gov) or cn-[a-z]+-[0-9]+ (China)
    /// See: https://docs.aws.amazon.com/general/latest/gr/rande.html
    pub region: String,
    /// AWS authentication configuration. If not specified, defaults to IRSA (IAM Roles for Service Accounts) - recommended.
    #[serde(default)]
    pub auth: Option<AwsAuthConfig>,
}

/// Azure configuration for Key Vault
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AzureConfig {
    /// Azure Key Vault name
    pub vault_name: String,
    /// Azure location/region for Key Vault (e.g., "eastus", "westus2", "southeastasia")
    /// Required: Must be specified for all Azure configurations
    /// Format: [direction][region][number] (e.g., eastus, westus2)
    /// See: https://azure.microsoft.com/en-us/explore/global-infrastructure/geographies/
    pub location: String,
    /// Azure authentication configuration. If not specified, defaults to Workload Identity (recommended).
    #[serde(default)]
    pub auth: Option<AzureAuthConfig>,
}

/// GCP authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum GcpAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires GKE cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        /// GCP service account email to impersonate
        /// Format: <service-account-name>@<project-id>.iam.gserviceaccount.com
        #[serde(rename = "serviceAccountEmail")]
        service_account_email: String,
    },
}

impl JsonSchema for GcpAuthConfig {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("GcpAuthConfig")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        // Use a single schema instead of oneOf to avoid nullable issues
        let schema_value = serde_json::json!({
            "type": "object",
            "description": "GCP authentication configuration - only supports Workload Identity",
            "properties": {
                "authType": {
                    "type": "string",
                    "enum": ["workloadIdentity"],
                    "description": "Authentication type: 'workloadIdentity' for Workload Identity"
                },
                "serviceAccountEmail": {
                    "type": "string",
                    "description": "GCP service account email to impersonate. Format: <service-account-name>@<project-id>.iam.gserviceaccount.com"
                }
            },
            "required": ["authType", "serviceAccountEmail"]
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for GcpAuthConfig")
    }
}

/// AWS authentication configuration
/// Only supports IRSA (IAM Roles for Service Accounts) - recommended and default
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AwsAuthConfig {
    /// Use IRSA (IAM Roles for Service Accounts) for authentication (DEFAULT)
    /// Requires EKS cluster with IRSA enabled and service account annotation
    /// This is the recommended authentication method and is used by default when auth is not specified
    Irsa {
        /// AWS IAM role ARN to assume
        /// Format: arn:aws:iam::<account-id>:role/<role-name>
        #[serde(rename = "roleArn")]
        role_arn: String,
    },
}

impl JsonSchema for AwsAuthConfig {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("AwsAuthConfig")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        // Use a single schema instead of oneOf to avoid nullable issues
        let schema_value = serde_json::json!({
            "type": "object",
            "description": "AWS authentication configuration - only supports IRSA (IAM Roles for Service Accounts)",
            "properties": {
                "authType": {
                    "type": "string",
                    "enum": ["irsa"],
                    "description": "Authentication type: 'irsa' for IAM Roles for Service Accounts"
                },
                "roleArn": {
                    "type": "string",
                    "description": "AWS IAM role ARN to assume. Format: arn:aws:iam::<account-id>:role/<role-name>"
                }
            },
            "required": ["authType", "roleArn"]
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for AwsAuthConfig")
    }
}

/// Azure authentication configuration
/// Only supports Workload Identity (recommended and default)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum AzureAuthConfig {
    /// Use Workload Identity for authentication (DEFAULT)
    /// Requires AKS cluster with Workload Identity enabled
    /// This is the recommended authentication method and is used by default when auth is not specified
    WorkloadIdentity {
        /// Azure service principal client ID
        #[serde(rename = "clientId")]
        client_id: String,
    },
}

impl JsonSchema for AzureAuthConfig {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("AzureAuthConfig")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        // Use a single schema instead of oneOf to avoid nullable issues
        let schema_value = serde_json::json!({
            "type": "object",
            "description": "Azure authentication configuration - only supports Workload Identity",
            "properties": {
                "authType": {
                    "type": "string",
                    "enum": ["workloadIdentity"],
                    "description": "Authentication type: 'workloadIdentity' for Workload Identity"
                },
                "clientId": {
                    "type": "string",
                    "description": "Azure service principal client ID"
                }
            },
            "required": ["authType", "clientId"]
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for AzureAuthConfig")
    }
}

/// OpenBao configuration for the KV v2 secrets engine.
///
/// Unlike the cloud providers, OpenBao is a single in-cluster endpoint with no
/// region/location concept, so no `location` field is present.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenBaoConfig {
    /// OpenBao API base URL, e.g. `https://openbao.openbao.svc:8200`.
    pub address: String,
    /// KV v2 mount name. Defaults to `secret`.
    #[serde(default = "default_openbao_mount")]
    pub mount: String,
    /// Optional OpenBao namespace (sent as `X-Vault-Namespace`).
    #[serde(default)]
    pub namespace: Option<String>,
    /// Optional reference to a Kubernetes Secret holding a custom CA for TLS.
    #[serde(default)]
    pub tls_ca_secret_ref: Option<SecretKeyRef>,
    /// Authentication configuration. Kubernetes auth is required.
    #[serde(default)]
    pub auth: Option<OpenBaoAuthConfig>,
}

/// Default OpenBao KV v2 mount name.
pub fn default_openbao_mount() -> String {
    "secret".to_string()
}

/// Reference to a single key within a Kubernetes Secret.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecretKeyRef {
    /// Name of the Kubernetes Secret.
    pub name: String,
    /// Key within the Secret's data.
    pub key: String,
}

/// OpenBao authentication configuration.
///
/// Only the Kubernetes auth method is supported (recommended for in-cluster
/// workloads): the controller's ServiceAccount JWT is exchanged for a
/// short-lived OpenBao token.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "authType")]
pub enum OpenBaoAuthConfig {
    /// Authenticate via the OpenBao Kubernetes auth method.
    Kubernetes {
        /// OpenBao Kubernetes auth role bound to the controller ServiceAccount.
        role: String,
        /// Path to the projected ServiceAccount token. Defaults to the standard
        /// in-pod token path when omitted.
        #[serde(default, rename = "serviceAccountTokenPath")]
        service_account_token_path: Option<String>,
    },
}

impl JsonSchema for OpenBaoAuthConfig {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("OpenBaoAuthConfig")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        // Single schema (not oneOf) to avoid nullable issues, matching the other
        // provider auth configs.
        let schema_value = serde_json::json!({
            "type": "object",
            "description": "OpenBao authentication configuration - only supports the Kubernetes auth method",
            "properties": {
                "authType": {
                    "type": "string",
                    "enum": ["kubernetes"],
                    "description": "Authentication type: 'kubernetes' for the OpenBao Kubernetes auth method"
                },
                "role": {
                    "type": "string",
                    "description": "OpenBao Kubernetes auth role bound to the controller ServiceAccount"
                },
                "serviceAccountTokenPath": {
                    "type": "string",
                    "description": "Path to the projected ServiceAccount token (defaults to the standard in-pod path)"
                }
            },
            "required": ["authType", "role"]
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for OpenBaoAuthConfig")
    }
}

#[cfg(test)]
mod openbao_tests {
    use super::*;

    #[test]
    fn deserializes_openbao_with_default_mount() {
        let yaml = r"
openbao:
  address: https://openbao.openbao.svc:8200
  auth:
    authType: kubernetes
    role: secret-manager-controller
";
        let cfg: ProviderConfig = serde_yaml::from_str(yaml).expect("deserialize openbao");
        match cfg {
            ProviderConfig::OpenBao(ob) => {
                assert_eq!(ob.address, "https://openbao.openbao.svc:8200");
                assert_eq!(ob.mount, "secret"); // defaulted
                match ob.auth {
                    Some(OpenBaoAuthConfig::Kubernetes { role, .. }) => {
                        assert_eq!(role, "secret-manager-controller");
                    }
                    None => panic!("expected kubernetes auth"),
                }
            }
            _ => panic!("expected OpenBao variant"),
        }
    }

    #[test]
    fn rejects_multiple_provider_types() {
        let yaml = r"
openbao:
  address: https://openbao:8200
gcp:
  projectId: my-project
  location: us-central1
";
        match serde_yaml::from_str::<ProviderConfig>(yaml) {
            Ok(_) => panic!("multiple provider types should be rejected"),
            Err(_) => {}
        }
    }
}
