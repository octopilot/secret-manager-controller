//! # SOPS Key Management
//!
//! Handles loading, reloading, and watching SOPS private keys from Kubernetes secrets.

use crate::controller::reconciler::types::Reconciler;
use anyhow::Result;
use kube::Client;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Load SOPS private key from Kubernetes secret in controller namespace
/// Defaults to microscaler-system namespace
pub async fn load_sops_private_key(client: &Client) -> Result<Option<String>> {
    use k8s_openapi::api::core::v1::Secret;
    use kube::Api;

    // Use controller namespace (defaults to microscaler-system)
    // Can be overridden via POD_NAMESPACE environment variable
    let namespace =
        std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "microscaler-system".to_string());

    let secrets: Api<Secret> = Api::namespaced(client.clone(), &namespace);

    // Try to get the SOPS private key secret
    // Expected secret name: sops-private-key (or similar)
    let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

    for secret_name in secret_names {
        match secrets.get(secret_name).await {
            Ok(secret) => {
                // Extract private key from secret data
                // The key might be in different fields: "private-key", "key", "gpg-key", etc.
                if let Some(ref data_map) = secret.data {
                    if let Some(data) = data_map
                        .get("private-key")
                        .or_else(|| data_map.get("key"))
                        .or_else(|| data_map.get("gpg-key"))
                    {
                        let key = String::from_utf8(data.0.clone())
                            .map_err(|e| anyhow::anyhow!("Failed to decode private key: {e}"))?;
                        info!("Loaded SOPS private key from secret: {}", secret_name);
                        return Ok(Some(key));
                    }
                }
            }
            Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                // Try next secret name
            }
            Err(e) => {
                warn!("Failed to get secret {}: {}", secret_name, e);
            }
        }
    }

    warn!(
        "SOPS private key not found in {} namespace, SOPS decryption will be disabled",
        namespace
    );
    Ok(None)
}

/// Reload SOPS private key from Kubernetes secret
/// Called when the secret changes to hot-reload the key without restarting
pub async fn reload_sops_private_key(reconciler: &Reconciler) -> Result<()> {
    let new_key = load_sops_private_key(&reconciler.client).await?;
    let mut key_guard = reconciler.sops_private_key.lock().await;
    *key_guard = new_key.clone();

    // Update capability flag based on whether key was loaded
    reconciler
        .sops_capability_ready
        .store(new_key.is_some(), std::sync::atomic::Ordering::Relaxed);

    if new_key.is_some() {
        info!("✅ SOPS capability ready - key reloaded from controller namespace");
    } else {
        warn!("⚠️  SOPS capability disabled - key removed from controller namespace");
    }

    Ok(())
}

/// Reload SOPS private key from a specific namespace
/// Falls back to controller namespace if not found
pub async fn reload_sops_private_key_from_namespace(
    reconciler: &Reconciler,
    namespace: &str,
) -> Result<()> {
    use k8s_openapi::api::core::v1::Secret;
    use kube::Api;

    let secrets: Api<Secret> = Api::namespaced(reconciler.client.clone(), namespace);
    let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

    for secret_name in secret_names {
        match secrets.get(secret_name).await {
            Ok(secret) => {
                if let Some(ref data_map) = secret.data {
                    if let Some(data) = data_map
                        .get("private-key")
                        .or_else(|| data_map.get("key"))
                        .or_else(|| data_map.get("gpg-key"))
                    {
                        let key = String::from_utf8(data.0.clone())
                            .map_err(|e| anyhow::anyhow!("Failed to decode private key: {e}"))?;
                        let mut key_guard = reconciler.sops_private_key.lock().await;
                        *key_guard = Some(key);

                        // Update capability flag if this is controller namespace
                        let controller_namespace = std::env::var("POD_NAMESPACE")
                            .unwrap_or_else(|_| "microscaler-system".to_string());
                        if namespace == controller_namespace {
                            reconciler
                                .sops_capability_ready
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            info!("✅ SOPS capability restored - key added back to controller namespace");
                        }

                        info!(
                            "✅ Reloaded SOPS private key from secret '{}/{}'",
                            namespace, secret_name
                        );
                        return Ok(());
                    }
                }
            }
            Err(kube::Error::Api(api_err)) if api_err.code == 404 => {
                // Try next secret name
            }
            Err(e) => {
                warn!(
                    "Failed to get secret '{}/{}': {}",
                    namespace, secret_name, e
                );
            }
        }
    }

    // CRITICAL: SOPS key not found in the resource namespace
    // This is a configuration error that SREs need to fix immediately
    // DO NOT silently fall back - this masks configuration problems
    error!(
        "🚨 CRITICAL: SOPS private key secret NOT FOUND in namespace '{}'",
        namespace
    );
    error!(
        "🚨 Expected secret names: 'sops-private-key', 'sops-gpg-key', or 'gpg-key' in namespace '{}'",
        namespace
    );
    error!(
        "🚨 ACTION REQUIRED: Create the SOPS private key secret in namespace '{}' to enable SOPS decryption",
        namespace
    );
    error!("🚨 Secret must contain one of these keys: 'private-key', 'key', or 'gpg-key'");
    error!(
        "🚨 This is a configuration error - SOPS decryption will FAIL for resources in namespace '{}'",
        namespace
    );

    // Return error instead of falling back - this forces SREs to fix the configuration
    Err(anyhow::anyhow!(
        "SOPS private key secret not found in namespace '{}'. Expected one of: 'sops-private-key', 'sops-gpg-key', or 'gpg-key' with key 'private-key', 'key', or 'gpg-key'. This is a configuration error that must be fixed.",
        namespace
    ))
}

/// Verify RBAC is properly configured for SOPS key watch
/// Tests actual API access to verify RBAC permissions are active
/// We test the actual operations we need rather than checking RBAC resources exist
/// (which would require clusterrole read permissions we shouldn't have)
pub async fn verify_rbac_for_sops_watch(client: &kube::Client) -> Result<()> {
    use k8s_openapi::api::core::v1::Secret;
    use kube::Api;

    // Test actual API access to verify RBAC is propagated
    // This is the real test - can we actually list secrets across all namespaces?
    // This is what we need for SOPS key watching, so if this works, RBAC is correct
    let secrets: Api<Secret> = Api::all(client.clone());
    match secrets
        .list(&kube::api::ListParams::default().limit(1))
        .await
    {
        Ok(_) => {
            debug!("✅ RBAC permissions verified - can list secrets across all namespaces");
            Ok(())
        }
        Err(e) => {
            // RBAC permissions not active - this could be propagation delay or misconfiguration
            Err(anyhow::anyhow!(
                "Cannot list secrets across all namespaces: {}. Verify RBAC is installed and ServiceAccount is bound to ClusterRole. Restart the pod if RBAC was created after pod started.",
                e
            ))
        }
    }
}

/// Start watching for SOPS private key secret changes across all namespaces
/// Spawns a background task that watches for secret updates and reloads the key
/// Watches all namespaces to detect SOPS secret changes in tilt, dev, stage, prod, etc.
pub fn start_sops_key_watch(reconciler: Arc<Reconciler>) {
    tokio::spawn(async move {
        use futures::pin_mut;
        use futures::StreamExt;
        use k8s_openapi::api::core::v1::Secret;
        use kube::Api;
        use kube_runtime::watcher;

        // Watch secrets across ALL namespaces to detect SOPS key changes everywhere
        let secrets: Api<Secret> = Api::all(reconciler.client.clone());

        // Watch for secrets matching SOPS key names
        let secret_names = vec!["sops-private-key", "sops-gpg-key", "gpg-key"];

        info!("Starting watch for SOPS private key secrets across all namespaces");

        // Verify RBAC is properly configured by testing actual API access
        // We test the operations we need (list secrets) rather than checking RBAC resources
        // This avoids requiring clusterrole read permissions we shouldn't have
        match verify_rbac_for_sops_watch(&reconciler.client).await {
            Ok(_) => {
                info!("✅ RBAC permissions verified - can list secrets across all namespaces");
            }
            Err(e) => {
                // Log warning but continue - controller will still work, just without SOPS hot-reload
                warn!(
                    "⚠️  RBAC verification failed: {}. SOPS key watch will not be started.",
                    e
                );
                warn!("⚠️  Controller will still work but SOPS key changes won't be hot-reloaded.");
                warn!("⚠️  To enable SOPS key hot-reloading:");
                warn!("     1. Verify RBAC is installed: kubectl get clusterrole secret-manager-controller");
                warn!("     2. Verify ServiceAccount is bound: kubectl get clusterrolebinding secret-manager-controller");
                warn!("     3. Test permissions: kubectl auth can-i list secrets --as=system:serviceaccount:microscaler-system:secret-manager-controller --all-namespaces");
                warn!("     4. If RBAC was created after pod started, restart the pod");
                return;
            }
        }

        // Watch all secrets in all namespaces and filter for SOPS key names
        // watcher() returns a Stream - pin it to use with StreamExt
        let stream = watcher(secrets, watcher::Config::default());
        pin_mut!(stream);

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    // Match on Event variants - handle all variants including Init events
                    match event {
                        watcher::Event::Apply(secret) => {
                            let secret_name = secret.metadata.name.as_deref().unwrap_or("unknown");
                            let secret_namespace =
                                secret.metadata.namespace.as_deref().unwrap_or("unknown");

                            // Check if this is one of the SOPS key secrets
                            if secret_names.contains(&secret_name) {
                                info!(
                                    "SOPS private key secret '{}/{}' changed, reloading...",
                                    secret_namespace, secret_name
                                );

                                // Update bootstrap flag if this is controller namespace
                                let controller_namespace = std::env::var("POD_NAMESPACE")
                                    .unwrap_or_else(|_| "microscaler-system".to_string());
                                if secret_namespace == controller_namespace {
                                    reconciler
                                        .sops_capability_ready
                                        .store(true, std::sync::atomic::Ordering::Relaxed);
                                    info!("✅ SOPS capability restored - key added back to controller namespace");
                                }

                                // Reload from the namespace where the secret changed
                                if let Err(e) = reload_sops_private_key_from_namespace(
                                    &reconciler,
                                    secret_namespace,
                                )
                                .await
                                {
                                    error!(
                                        "Failed to reload SOPS private key from namespace {}: {}",
                                        secret_namespace, e
                                    );
                                } else {
                                    // Update status for all resources in this namespace
                                    if let Err(e) = crate::controller::reconciler::status::update_all_resources_in_namespace(
                                        &reconciler,
                                        secret_namespace,
                                        true, // key_available
                                        Some(secret_name.to_string()),
                                    )
                                    .await
                                    {
                                        warn!(
                                            "Failed to update SOPS key status for resources in namespace {}: {}",
                                            secret_namespace, e
                                        );
                                    } else {
                                        info!(
                                            "✅ SOPS key available in namespace '{}' - updated all SecretManagerConfig resources",
                                            secret_namespace
                                        );
                                    }
                                }
                            }
                        }
                        watcher::Event::Delete(secret) => {
                            let secret_name = secret.metadata.name.as_deref().unwrap_or("unknown");
                            let secret_namespace =
                                secret.metadata.namespace.as_deref().unwrap_or("unknown");
                            if secret_names.contains(&secret_name) {
                                warn!(
                                    "SOPS private key secret '{}/{}' was deleted",
                                    secret_namespace, secret_name
                                );

                                // Update bootstrap flag if this is controller namespace
                                let controller_namespace = std::env::var("POD_NAMESPACE")
                                    .unwrap_or_else(|_| "microscaler-system".to_string());
                                if secret_namespace == controller_namespace {
                                    reconciler
                                        .sops_capability_ready
                                        .store(false, std::sync::atomic::Ordering::Relaxed);
                                    warn!("⚠️  SOPS capability disabled - key removed from controller namespace");
                                }

                                // Try to reload from controller namespace as fallback
                                if let Err(e) = reload_sops_private_key(&reconciler).await {
                                    warn!("Failed to reload SOPS private key from controller namespace: {}", e);
                                    // Clear the key if reload fails
                                    let mut key_guard = reconciler.sops_private_key.lock().await;
                                    *key_guard = None;
                                    // Update capability flag
                                    reconciler
                                        .sops_capability_ready
                                        .store(false, std::sync::atomic::Ordering::Relaxed);
                                    warn!("⚠️  SOPS capability disabled - key cleared");
                                }

                                // Update status for all resources in this namespace
                                if let Err(e) = crate::controller::reconciler::status::update_all_resources_in_namespace(
                                    &reconciler,
                                    secret_namespace,
                                    false, // key_available
                                    None, // secret_name (no longer exists)
                                )
                                .await
                                {
                                    warn!(
                                        "Failed to update SOPS key status for resources in namespace {}: {}",
                                        secret_namespace, e
                                    );
                                } else {
                                    warn!(
                                        "⚠️  SOPS key removed from namespace '{}' - updated all SecretManagerConfig resources",
                                        secret_namespace
                                    );
                                }
                            }
                        }
                        watcher::Event::Init
                        | watcher::Event::InitApply(_)
                        | watcher::Event::InitDone => {
                            // Initial watch events - ignore, we already loaded the key at startup
                        }
                    }
                }
                Err(e) => {
                    warn!("Error watching SOPS key secrets: {}", e);
                    // Continue watching - errors are transient
                }
            }
        }

        warn!("SOPS key secret watch stream ended");
    });
}
