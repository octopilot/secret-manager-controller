//! # Hot Reload Configuration
//!
//! Configuration for hot-reloading controller settings from ConfigMap.

use serde::{Deserialize, Serialize};

/// Hot reload configuration
///
/// Controls whether the controller watches for ConfigMap changes and hot-reloads configuration.
/// When enabled, the controller watches the specified ConfigMap and reloads configuration
/// without requiring a pod restart.
///
/// **Note**: If you use a tool like Reloader that automatically restarts pods when ConfigMaps
/// change, hot-reload may be redundant. However, hot-reload avoids pod restarts and provides
/// faster configuration updates.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HotReloadConfig {
    /// Enable hot-reload of controller configuration
    /// When true, watches ConfigMap for changes and reloads configuration without restart
    /// When false, configuration is only loaded at startup
    /// Default: false (disabled) - most users rely on pod restarts via Reloader or manual updates
    #[serde(default = "default_false")]
    pub enabled: bool,
    /// ConfigMap name to watch for configuration changes
    /// The ConfigMap should be in the same namespace as the controller
    /// Environment variables are populated from this ConfigMap using `envFrom` in the deployment
    /// Default: "secret-manager-controller-config"
    #[serde(default = "default_configmap_name")]
    pub config_map_name: String,
    /// ConfigMap namespace
    /// Namespace where the ConfigMap is located
    /// If not specified, uses the controller's namespace (from POD_NAMESPACE env var)
    #[serde(default)]
    pub config_map_namespace: Option<String>,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            config_map_name: "secret-manager-controller-config".to_string(),
            config_map_namespace: None,
        }
    }
}

/// Default value for boolean false
fn default_false() -> bool {
    false
}

/// Default ConfigMap name
fn default_configmap_name() -> String {
    "secret-manager-controller-config".to_string()
}
