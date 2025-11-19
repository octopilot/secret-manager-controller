//! # SecretManagerConfig Validation
//!
//! Main orchestration for validating SecretManagerConfig resources.

use crate::crd::SecretManagerConfig;
use anyhow::Result;

use super::configs::validate_configs_config;
use super::kubernetes::{
    validate_kubernetes_label, validate_kubernetes_name, validate_kubernetes_namespace,
    validate_source_ref_kind,
};
use super::paths::validate_path;
use super::provider::validate_provider_config;
use super::secrets::validate_secret_name_component;

/// Validate SecretManagerConfig resource
/// Performs comprehensive validation of all CRD fields
pub fn validate_secret_manager_config(config: &SecretManagerConfig) -> Result<()> {
    // Validate sourceRef.kind
    if config.spec.source_ref.kind.is_empty() {
        return Err(anyhow::anyhow!("sourceRef.kind is required but is empty"));
    }
    if let Err(e) = validate_source_ref_kind(&config.spec.source_ref.kind) {
        return Err(anyhow::anyhow!(
            "Invalid sourceRef.kind '{}': {}",
            config.spec.source_ref.kind,
            e
        ));
    }

    // Validate sourceRef.name
    if config.spec.source_ref.name.is_empty() {
        return Err(anyhow::anyhow!("sourceRef.name is required but is empty"));
    }
    if let Err(e) = validate_kubernetes_name(&config.spec.source_ref.name, "sourceRef.name") {
        return Err(anyhow::anyhow!(
            "Invalid sourceRef.name '{}': {}",
            config.spec.source_ref.name,
            e
        ));
    }

    // Validate sourceRef.namespace
    if config.spec.source_ref.namespace.is_empty() {
        return Err(anyhow::anyhow!(
            "sourceRef.namespace is required but is empty"
        ));
    }
    if let Err(e) = validate_kubernetes_namespace(&config.spec.source_ref.namespace) {
        return Err(anyhow::anyhow!(
            "Invalid sourceRef.namespace '{}': {}",
            config.spec.source_ref.namespace,
            e
        ));
    }

    // Validate secrets.environment
    if config.spec.secrets.environment.is_empty() {
        return Err(anyhow::anyhow!(
            "secrets.environment is required but is empty"
        ));
    }
    if let Err(e) =
        validate_kubernetes_label(&config.spec.secrets.environment, "secrets.environment")
    {
        return Err(anyhow::anyhow!(
            "Invalid secrets.environment '{}': {}",
            config.spec.secrets.environment,
            e
        ));
    }

    // Validate optional secrets fields
    if let Some(ref prefix) = config.spec.secrets.prefix {
        if !prefix.is_empty() {
            if let Err(e) = validate_secret_name_component(prefix, "secrets.prefix") {
                return Err(anyhow::anyhow!("Invalid secrets.prefix '{prefix}': {e}"));
            }
        }
    }

    if let Some(ref suffix) = config.spec.secrets.suffix {
        if !suffix.is_empty() {
            if let Err(e) = validate_secret_name_component(suffix, "secrets.suffix") {
                return Err(anyhow::anyhow!("Invalid secrets.suffix '{suffix}': {e}"));
            }
        }
    }

    if let Some(ref base_path) = config.spec.secrets.base_path {
        if !base_path.is_empty() {
            if let Err(e) = validate_path(base_path, "secrets.basePath") {
                return Err(anyhow::anyhow!(
                    "Invalid secrets.basePath '{base_path}': {e}"
                ));
            }
        }
    }

    if let Some(ref kustomize_path) = config.spec.secrets.kustomize_path {
        if !kustomize_path.is_empty() {
            if let Err(e) = validate_path(kustomize_path, "secrets.kustomizePath") {
                return Err(anyhow::anyhow!(
                    "Invalid secrets.kustomizePath '{kustomize_path}': {e}"
                ));
            }
        }
    }

    // Validate provider configuration
    if let Err(e) = validate_provider_config(&config.spec.provider) {
        return Err(anyhow::anyhow!("Invalid provider configuration: {e}"));
    }

    // Validate configs configuration if present
    if let Some(ref configs) = config.spec.configs {
        if let Err(e) = validate_configs_config(configs) {
            return Err(anyhow::anyhow!("Invalid configs configuration: {e}"));
        }
    }

    // Boolean fields are validated by serde, but we ensure they're not None
    // diffDiscovery and triggerUpdate have defaults, so they're always present

    Ok(())
}
