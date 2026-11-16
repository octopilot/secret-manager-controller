//! # SOPS Key Loading
//!
//! Functions for loading and reloading SOPS private keys from Kubernetes secrets.
//!
//! A namespace's key material is everything found in its key Secrets: an age identity,
//! an armored GPG private key, or both. Holding both lets one controller decrypt a
//! repository whose files are encrypted to age recipients and to GPG keys.

use crate::controller::parser::sops::gpg::split_sops_keys;
use crate::controller::reconciler::types::Reconciler;
use anyhow::Result;
use k8s_openapi::api::core::v1::Secret;
use kube::{Api, Client};
use tracing::{error, info, warn};

/// Names of the Secrets a SOPS private key is read from.
pub const KEY_SECRET_NAMES: [&str; 3] = ["sops-private-key", "sops-gpg-key", "gpg-key"];

/// Fields of those Secrets that hold key material. Every one present is used.
const KEY_FIELDS: [&str; 5] = ["private-key", "key", "age-key", "gpg-key", "pgp-key"];

/// All the SOPS key material of `namespace`: the values of every key field of every
/// key Secret there, joined. `None` when the namespace has none.
async fn read_namespace_keys(client: &Client, namespace: &str) -> Result<Option<String>> {
    let secrets: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let mut values: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();

    for secret_name in KEY_SECRET_NAMES {
        match secrets.get(secret_name).await {
            Ok(secret) => {
                let Some(data) = secret.data else { continue };
                let mut found = false;
                for field in KEY_FIELDS {
                    let Some(value) = data.get(field) else {
                        continue;
                    };
                    let value = String::from_utf8(value.0.clone()).map_err(|e| {
                        anyhow::anyhow!(
                            "SOPS private key '{namespace}/{secret_name}' field '{field}' is not text: {e}"
                        )
                    })?;
                    found = true;
                    sources.push(format!("{secret_name}/{field}"));
                    if !values.contains(&value) {
                        values.push(value);
                    }
                }
                if !found {
                    warn!(
                        "Secret '{}/{}' found but has none of the key fields ({})",
                        namespace,
                        secret_name,
                        KEY_FIELDS.join(", ")
                    );
                }
            }
            Err(kube::Error::Api(api_err)) if api_err.code == 404 => {}
            Err(e) => warn!(
                "Failed to get secret '{}/{}': {}",
                namespace, secret_name, e
            ),
        }
    }

    if values.is_empty() {
        return Ok(None);
    }
    let key = values.join("\n");

    let material = split_sops_keys(&key);
    let has_gpg_block = key.contains("-----BEGIN PGP PRIVATE KEY BLOCK-----")
        && key.contains("-----END PGP PRIVATE KEY BLOCK-----");
    let kinds = match (material.age.is_some(), has_gpg_block) {
        (true, true) => "age identity and armored GPG key",
        (true, false) => "age identity",
        (false, true) => "armored GPG key",
        (false, false) => "unrecognised",
    };
    info!(
        "✅ Loaded SOPS private key of namespace '{}' from {}: {} ({} bytes)",
        namespace,
        sources.join(", "),
        kinds,
        key.len()
    );
    if material.age.is_none() && !has_gpg_block {
        warn!(
            "⚠️  SOPS private key of namespace '{}' may be malformed: neither an age identity nor an armored GPG key",
            namespace
        );
    }

    Ok(Some(key))
}

/// Load the SOPS private key of the controller namespace (`POD_NAMESPACE`).
pub async fn load_sops_private_key(client: &Client) -> Result<Option<String>> {
    let namespace = crate::controller::reconciler::sops::controller_namespace();
    let key = read_namespace_keys(client, &namespace).await?;
    if key.is_none() {
        warn!(
            "SOPS private key not found in {} namespace, SOPS decryption will be disabled",
            namespace
        );
    }
    Ok(key)
}

/// Reload the controller namespace's SOPS private key.
pub async fn reload_sops_private_key(reconciler: &Reconciler) -> Result<()> {
    let new_key = load_sops_private_key(&reconciler.client).await?;
    let controller_namespace = crate::controller::reconciler::sops::controller_namespace();
    {
        let mut keys = reconciler.sops_keys.lock().await;
        match &new_key {
            Some(key) => keys.set(&controller_namespace, key.clone()),
            None => {
                keys.remove(&controller_namespace);
            }
        }
    }

    reconciler
        .sops_capability_ready
        .store(new_key.is_some(), std::sync::atomic::Ordering::Relaxed);

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

/// Reload the SOPS private key of `namespace` from its key Secrets.
///
/// Only that namespace's key changes: resources elsewhere keep theirs. An error when the
/// namespace has no key Secret, so the caller can tell "no key" from "key loaded".
pub async fn reload_sops_private_key_from_namespace(
    reconciler: &Reconciler,
    namespace: &str,
) -> Result<()> {
    let Some(key) = read_namespace_keys(&reconciler.client, namespace).await? else {
        error!(
            "SOPS private key secret not found in namespace '{}': expected one of {} with a field among {}",
            namespace,
            KEY_SECRET_NAMES.join(", "),
            KEY_FIELDS.join(", ")
        );
        return Err(anyhow::anyhow!(
            "SOPS private key secret not found in namespace '{namespace}'. Expected one of: {} with a field among: {}.",
            KEY_SECRET_NAMES.join(", "),
            KEY_FIELDS.join(", ")
        ));
    };

    reconciler.sops_keys.lock().await.set(namespace, key);

    if namespace == crate::controller::reconciler::sops::controller_namespace() {
        reconciler
            .sops_capability_ready
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    info!("✅ Reloaded SOPS private key of namespace '{}'", namespace);
    Ok(())
}
