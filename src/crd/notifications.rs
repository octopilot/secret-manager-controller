//! # Notification Configuration
//!
//! Configuration for sending notifications when drift is detected.
//! Supports both FluxCD and ArgoCD notification mechanisms.

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Notification configuration for drift detection alerts
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationConfig {
    /// FluxCD notification configuration (for GitRepository sources)
    /// When set, creates a FluxCD Alert CRD that watches this SecretManagerConfig
    /// and sends notifications via the specified Provider when drift is detected
    #[serde(default)]
    pub fluxcd: Option<FluxCDNotificationConfig>,
    /// ArgoCD notification configuration (for Application sources)
    /// When set, adds annotations to the ArgoCD Application resource
    /// to trigger notifications when drift is detected
    #[serde(default)]
    pub argocd: Option<ArgoCDNotificationConfig>,
}

/// FluxCD notification configuration
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FluxCDNotificationConfig {
    /// FluxCD Provider reference
    pub provider_ref: ProviderRef,
}

/// FluxCD Provider reference
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRef {
    /// Name of the FluxCD Provider resource
    pub name: String,
    /// Namespace of the FluxCD Provider resource
    /// Defaults to the same namespace as the SecretManagerConfig
    #[serde(default)]
    pub namespace: Option<String>,
}

/// ArgoCD notification configuration
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArgoCDNotificationConfig {
    /// List of notification subscriptions
    /// Each subscription defines a trigger, service, and channel
    pub subscriptions: Vec<NotificationSubscription>,
}

/// Notification subscription for ArgoCD
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSubscription {
    /// Notification trigger name (e.g., "drift-detected")
    pub trigger: String,
    /// Notification service (e.g., "slack", "email", "webhook")
    pub service: String,
    /// Notification channel (e.g., "#secrets-alerts" for Slack, "team@example.com" for email)
    pub channel: String,
}

impl JsonSchema for NotificationConfig {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("NotificationConfig")
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        // Use a custom schema that explicitly marks fluxcd and argocd as nullable
        // This ensures they are optional in the Kubernetes CRD schema
        // Note: FluxCD and ArgoCD notifications are configured independently:
        // - FluxCD shops only need fluxcd.providerRef (for GitRepository sources)
        // - ArgoCD shops only need argocd (for Application sources)
        // - Both can be configured if using both GitOps tools
        let schema_value = serde_json::json!({
            "type": "object",
            "description": "Notification configuration for drift detection alerts. Supports both FluxCD (via Provider reference) and ArgoCD (via Application annotations). FluxCD and ArgoCD notifications are configured independently - FluxCD shops only need fluxcd, ArgoCD shops only need argocd. Both can be configured if using both GitOps tools.",
            "properties": {
                "fluxcd": {
                    "type": "object",
                    "nullable": true,
                    "description": "FluxCD notification configuration (for GitRepository sources). Optional. When set, creates a FluxCD Alert CRD that watches this SecretManagerConfig and sends notifications via the specified Provider when drift is detected.",
                    "properties": {
                        "providerRef": {
                            "type": "object",
                            "description": "FluxCD Provider reference",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Name of the FluxCD Provider resource"
                                },
                                "namespace": {
                                    "type": "string",
                                    "nullable": true,
                                    "description": "Namespace of the FluxCD Provider resource. Optional. Defaults to the same namespace as the SecretManagerConfig."
                                }
                            },
                            "required": ["name"]
                        }
                    },
                    "required": ["providerRef"]
                },
                "argocd": {
                    "type": "object",
                    "nullable": true,
                    "description": "ArgoCD notification configuration (for Application sources). Optional. When set, adds annotations to the ArgoCD Application resource to trigger notifications when drift is detected.",
                    "properties": {
                        "subscriptions": {
                            "type": "array",
                            "description": "List of notification subscriptions",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "trigger": {
                                        "type": "string",
                                        "description": "Notification trigger name (e.g., 'drift-detected')"
                                    },
                                    "service": {
                                        "type": "string",
                                        "description": "Notification service (e.g., 'slack', 'email', 'webhook')"
                                    },
                                    "channel": {
                                        "type": "string",
                                        "description": "Notification channel (e.g., '#secrets-alerts' for Slack, 'team@example.com' for email)"
                                    }
                                },
                                "required": ["trigger", "service", "channel"]
                            }
                        }
                    },
                    "required": ["subscriptions"]
                }
            }
        });
        Schema::try_from(schema_value).expect("Failed to create Schema for NotificationConfig")
    }
}
