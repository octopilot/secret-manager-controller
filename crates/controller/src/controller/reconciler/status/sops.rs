//! # SOPS Key Status Management
//!
//! Handles checking and updating SOPS key availability status.

use crate::controller::reconciler::types::Reconciler;
use crate::crd::SecretManagerConfig;
use anyhow::Result;
use kube::Api;
use kube::api::PatchParams;
use tracing::debug;

/// Check SOPS key availability in a namespace
/// Returns (key_available, secret_name) tuple
pub async fn check_sops_key_availability(
    reconciler: &Reconciler,
    namespace: &str,
) -> Result<(bool, Option<String>)> {
    use k8s_openapi::api::core::v1::Secret;

    let secrets: Api<Secret> = Api::namespaced(reconciler.client.clone(), namespace);
    let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

    for secret_name in secret_names {
        match secrets.get(secret_name).await {
            Ok(secret) => {
                if let Some(ref data_map) = secret.data {
                    if data_map
                        .get("private-key")
                        .or_else(|| data_map.get("key"))
                        .or_else(|| data_map.get("gpg-key"))
                        .is_some()
                    {
                        return Ok((true, Some(secret_name.to_string())));
                    }
                }
            }
            Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                continue;
            }
            Err(e) => {
                debug!(
                    "Failed to check secret '{}/{}': {}",
                    namespace, secret_name, e
                );
            }
        }
    }

    Ok((false, None))
}

/// Update SOPS key status in SecretManagerConfig resource
pub async fn update_sops_key_status(
    reconciler: &Reconciler,
    config: &SecretManagerConfig,
    key_available: bool,
    secret_name: Option<String>,
) -> Result<()> {
    let api: Api<SecretManagerConfig> = Api::namespaced(
        reconciler.client.clone(),
        config.metadata.namespace.as_deref().unwrap_or("default"),
    );

    // Get existing status to preserve other fields
    let existing_status = config.status.as_ref();
    // Only the SOPS key fields: `config` may be a stale snapshot, and writing the whole
    // status back from it would undo newer writes (phase, next reconcile time, ...).
    let unchanged = existing_status.is_some_and(|existing| {
        existing.sops_key_available == Some(key_available)
            && existing.sops_key_secret_name == secret_name
            && existing.sops_key_namespace == config.metadata.namespace
    });
    if unchanged {
        return Ok(());
    }
    let patch = serde_json::json!({
        "status": {
            "sopsKeyAvailable": key_available,
            "sopsKeySecretName": secret_name,
            "sopsKeyNamespace": config.metadata.namespace,
            "sopsKeyLastChecked": chrono::Utc::now().to_rfc3339(),
        }
    });

    let resource_name = config.metadata.name.as_deref().unwrap_or("unknown");
    let resource_namespace = config.metadata.namespace.as_deref().unwrap_or("default");

    match api
        .patch_status(
            resource_name,
            &PatchParams::apply("secret-manager-controller"),
            &kube::api::Patch::Merge(patch),
        )
        .await
    {
        Ok(_) => {}
        Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
            // Resource was deleted during reconciliation - this is expected and not an error
            debug!(
                "SecretManagerConfig {}/{} was deleted during reconciliation, skipping SOPS key status update",
                resource_namespace, resource_name
            );
            return Ok(());
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to patch SOPS key status for SecretManagerConfig {}/{}: {}",
                resource_namespace,
                resource_name,
                e
            ));
        }
    }

    debug!(
        "Updated SOPS key status for SecretManagerConfig {}/{}: available={}",
        config.metadata.namespace.as_deref().unwrap_or("default"),
        config.metadata.name.as_deref().unwrap_or("unknown"),
        key_available
    );

    Ok(())
}

/// Update SOPS key status for all SecretManagerConfig resources in a namespace
/// Called by watch when SOPS key secret is created/deleted
pub async fn update_all_resources_in_namespace(
    reconciler: &Reconciler,
    namespace: &str,
    key_available: bool,
    secret_name: Option<String>,
) -> Result<()> {
    let api: Api<SecretManagerConfig> = Api::namespaced(reconciler.client.clone(), namespace);

    // List all SecretManagerConfig resources in this namespace
    let resources = api.list(&kube::api::ListParams::default()).await?;

    let mut updated_count = 0;
    let mut failed_count = 0;

    for resource in resources {
        match update_sops_key_status(reconciler, &resource, key_available, secret_name.clone())
            .await
        {
            Ok(_) => {
                updated_count += 1;
            }
            Err(e) => {
                failed_count += 1;
                debug!(
                    "Failed to update SOPS key status for {}/{}: {}",
                    namespace,
                    resource.metadata.name.as_deref().unwrap_or("unknown"),
                    e
                );
            }
        }
    }

    debug!(
        "Updated SOPS key status for {} resources in namespace '{}' ({} succeeded, {} failed)",
        updated_count + failed_count,
        namespace,
        updated_count,
        failed_count
    );

    Ok(())
}
