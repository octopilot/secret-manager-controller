//! # SOPS Key Watch Loop
//!
//! Watches for SOPS private key secret changes across all namespaces.

use crate::controller::reconciler::sops::load::reload_sops_private_key_from_namespace;
use crate::controller::reconciler::sops::rbac::verify_rbac_for_sops_watch;
use crate::controller::reconciler::types::Reconciler;
use futures::{pin_mut, StreamExt};
use k8s_openapi::api::core::v1::Secret;
use kube::Api;
use kube_runtime::watcher;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Start watching for SOPS private key secret changes across all namespaces
/// Spawns a background task that watches for secret updates and reloads the key
/// Watches all namespaces to detect SOPS secret changes in tilt, dev, stage, prod, etc.
pub fn start_sops_key_watch(reconciler: Arc<Reconciler>) {
    tokio::spawn(async move {
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
                warn!("     3. Test permissions: kubectl auth can-i list secrets --as=system:serviceaccount:octopilot-system:secret-manager-controller --all-namespaces");
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
                            handle_secret_apply(&reconciler, &secret, &secret_names).await;
                        }
                        watcher::Event::Delete(secret) => {
                            handle_secret_delete(&reconciler, &secret, &secret_names).await;
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

/// Handle secret apply event (create or update)
async fn handle_secret_apply(reconciler: &Reconciler, secret: &Secret, secret_names: &[&str]) {
    let secret_name = secret.metadata.name.as_deref().unwrap_or("unknown");
    let secret_namespace = secret.metadata.namespace.as_deref().unwrap_or("unknown");

    // Check if this is one of the SOPS key secrets
    if secret_names.contains(&secret_name) {
        info!(
            "SOPS private key secret '{}/{}' changed, reloading...",
            secret_namespace, secret_name
        );

        // Update bootstrap flag if this is controller namespace
        let controller_namespace =
            std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "octopilot-system".to_string());
        if secret_namespace == controller_namespace {
            reconciler
                .sops_capability_ready
                .store(true, std::sync::atomic::Ordering::Relaxed);
            info!(
                "✅ SOPS capability restored - key added back to controller namespace '{}'",
                secret_namespace
            );
        }

        // Reload from the namespace where the secret changed
        if let Err(e) = reload_sops_private_key_from_namespace(reconciler, secret_namespace).await {
            error!(
                "Failed to reload SOPS private key from namespace {}: {}",
                secret_namespace, e
            );
        } else {
            // Update status for all resources in this namespace
            if let Err(e) =
                crate::controller::reconciler::status::update_all_resources_in_namespace(
                    reconciler,
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

/// Handle secret delete event
async fn handle_secret_delete(reconciler: &Reconciler, secret: &Secret, secret_names: &[&str]) {
    let secret_name = secret.metadata.name.as_deref().unwrap_or("unknown");
    let secret_namespace = secret.metadata.namespace.as_deref().unwrap_or("unknown");

    if secret_names.contains(&secret_name) {
        warn!(
            "SOPS private key secret '{}/{}' was deleted",
            secret_namespace, secret_name
        );

        // Update bootstrap flag if this is controller namespace
        let controller_namespace =
            std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "octopilot-system".to_string());
        if secret_namespace == controller_namespace {
            reconciler
                .sops_capability_ready
                .store(false, std::sync::atomic::Ordering::Relaxed);
            warn!(
                "⚠️  SOPS capability disabled - key removed from controller namespace '{}'",
                secret_namespace
            );
        }

        // Try to reload from controller namespace as fallback
        let controller_namespace_for_reload =
            std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "octopilot-system".to_string());
        if let Err(e) =
            crate::controller::reconciler::sops::load::reload_sops_private_key(reconciler).await
        {
            warn!(
                "Failed to reload SOPS private key from controller namespace '{}': {}",
                controller_namespace_for_reload, e
            );
            // Clear the key if reload fails
            let mut key_guard = reconciler.sops_private_key.lock().await;
            *key_guard = None;
            // Update capability flag
            reconciler
                .sops_capability_ready
                .store(false, std::sync::atomic::Ordering::Relaxed);
            warn!(
                "⚠️  SOPS capability disabled - key cleared from controller namespace '{}'",
                controller_namespace_for_reload
            );
        }

        // Update status for all resources in this namespace
        if let Err(e) = crate::controller::reconciler::status::update_all_resources_in_namespace(
            reconciler,
            secret_namespace,
            false, // key_available
            None,  // secret_name (no longer exists)
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
