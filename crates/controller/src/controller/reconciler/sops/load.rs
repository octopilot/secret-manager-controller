//! # SOPS Key Loading
//!
//! Functions for loading and reloading SOPS private keys from Kubernetes secrets.

use crate::controller::reconciler::types::Reconciler;
use anyhow::Result;
use k8s_openapi::api::core::v1::Secret;
use kube::{Api, Client};
use tracing::{error, info, warn};

/// Load SOPS private key from Kubernetes secret in controller namespace
/// Defaults to octopilot-system namespace
pub async fn load_sops_private_key(client: &Client) -> Result<Option<String>> {
    // Use controller namespace (defaults to octopilot-system)
    // Can be overridden via POD_NAMESPACE environment variable
    let namespace =
        std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "octopilot-system".to_string());

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

                        // Validate key format
                        let key_size = key.len();
                        let has_pgp_header = key.contains("-----BEGIN PGP PRIVATE KEY BLOCK-----");
                        let has_pgp_footer = key.contains("-----END PGP PRIVATE KEY BLOCK-----");

                        info!(
                            "✅ Loaded SOPS private key from secret '{}/{}': size={} bytes, has_header={}, has_footer={}",
                            namespace, secret_name, key_size, has_pgp_header, has_pgp_footer
                        );

                        if !has_pgp_header || !has_pgp_footer {
                            warn!(
                                "⚠️  SOPS private key from '{}/{}' may be malformed: missing PGP headers/footers",
                                namespace, secret_name
                            );
                        }

                        return Ok(Some(key));
                    } else {
                        warn!(
                            "Secret '{}/{}' found but missing expected keys ('private-key', 'key', or 'gpg-key')",
                            namespace, secret_name
                        );
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

    let controller_namespace =
        std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "octopilot-system".to_string());

    if new_key.is_some() {
        info!(
            "✅ SOPS capability ready - key reloaded from controller namespace '{}'",
            controller_namespace
        );
    } else {
        warn!(
            "⚠️  SOPS capability disabled - key removed from controller namespace '{}'",
            controller_namespace
        );
    }

    Ok(())
}

/// Reload SOPS private key from a specific namespace
/// Falls back to controller namespace if not found
pub async fn reload_sops_private_key_from_namespace(
    reconciler: &Reconciler,
    namespace: &str,
) -> Result<()> {
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

                        // Validate key format
                        let key_size = key.len();
                        let has_pgp_header = key.contains("-----BEGIN PGP PRIVATE KEY BLOCK-----");
                        let has_pgp_footer = key.contains("-----END PGP PRIVATE KEY BLOCK-----");

                        info!(
                            "✅ Loaded SOPS private key from secret '{}/{}': size={} bytes, has_header={}, has_footer={}",
                            namespace, secret_name, key_size, has_pgp_header, has_pgp_footer
                        );

                        if !has_pgp_header || !has_pgp_footer {
                            warn!(
                                "⚠️  SOPS private key from '{}/{}' may be malformed: missing PGP headers/footers",
                                namespace, secret_name
                            );
                        }

                        let mut key_guard = reconciler.sops_private_key.lock().await;
                        *key_guard = Some(key);

                        // Update capability flag if this is controller namespace
                        let controller_namespace = std::env::var("POD_NAMESPACE")
                            .unwrap_or_else(|_| "octopilot-system".to_string());
                        if namespace == controller_namespace {
                            reconciler
                                .sops_capability_ready
                                .store(true, std::sync::atomic::Ordering::Relaxed);
                            info!("✅ SOPS capability restored - key added back to controller namespace '{}'", namespace);
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
