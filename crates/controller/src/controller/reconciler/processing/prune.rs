//! # Pruning
//!
//! Removing a key from a secret file does not, by default, remove its secret from the
//! provider: the controller only creates, updates, disables and enables. With the
//! annotation `secret-management.octopilot.io/prune: "true"` on a SecretManagerConfig,
//! secrets that config synced earlier and that are no longer in its files are deleted.
//!
//! Only secrets recorded in the config's own status are candidates. Nothing is found by
//! listing the provider, so a secret the config never wrote is never deleted, whatever
//! its name.

use crate::crd::{ResourceSyncState, SecretManagerConfig};
use crate::provider::SecretManagerProvider;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracing::{error, info, warn};

/// Annotation that turns pruning on for a SecretManagerConfig. Off unless `"true"`.
pub const PRUNE_ANNOTATION: &str = "secret-management.octopilot.io/prune";

/// Whether `config` asks for pruning.
#[must_use]
pub fn prune_enabled(config: &SecretManagerConfig) -> bool {
    config
        .metadata
        .annotations
        .as_ref()
        .and_then(|annotations| annotations.get(PRUNE_ANNOTATION))
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

/// Secrets recorded as synced by an earlier reconcile that are not wanted any more,
/// sorted by name.
#[must_use]
pub fn stale_secrets(
    previously_synced: &HashMap<String, ResourceSyncState>,
    desired: &HashSet<String>,
) -> Vec<String> {
    let mut stale: Vec<String> = previously_synced
        .iter()
        .filter(|(name, state)| state.exists && !desired.contains(*name))
        .map(|(name, _)| name.clone())
        .collect();
    stale.sort_unstable();
    stale
}

/// Delete from the provider the secrets `config` synced earlier that are no longer in
/// its files, and mark them as gone in `synced_secrets`. Returns how many were deleted.
///
/// Called only after every file of the config was read and synced without error, with
/// `desired` holding the name of every secret those files define (disabled ones
/// included: a commented-out key is disabled, not pruned).
pub async fn prune_secrets(
    provider: &dyn SecretManagerProvider,
    config: &SecretManagerConfig,
    desired: &HashSet<String>,
    synced_secrets: &mut HashMap<String, ResourceSyncState>,
) -> Result<u32> {
    let previously_synced = config
        .status
        .as_ref()
        .and_then(|status| status.sync.as_ref())
        .and_then(|sync| sync.secrets.clone())
        .unwrap_or_default();
    let stale = stale_secrets(&previously_synced, desired);
    if stale.is_empty() {
        return Ok(0);
    }

    // Files that yield no secret at all are far more likely a mistake (a wrong path, an
    // emptied file) than a wish to delete everything: refuse, loudly.
    if desired.is_empty() {
        warn!(
            "Pruning skipped: the files define no secrets, and pruning would delete all {} secrets this resource synced. Delete them by hand if that is intended.",
            stale.len()
        );
        return Ok(0);
    }

    let mut pruned = 0;
    let mut errors = Vec::new();
    for secret_name in stale {
        match provider.delete_secret(&secret_name).await {
            Ok(()) => {
                pruned += 1;
                info!(
                    "🗑️  Pruned secret '{}' (no longer in the secret files)",
                    secret_name
                );
                mark_gone(synced_secrets, &secret_name);
            }
            Err(e) if is_not_found(&e) => {
                info!("Secret '{}' is already gone from the provider", secret_name);
                mark_gone(synced_secrets, &secret_name);
            }
            Err(e) => {
                error!("Failed to prune secret {}: {}", secret_name, e);
                errors.push(format!("Failed to prune secret {secret_name}: {e}"));
            }
        }
    }

    if errors.is_empty() {
        Ok(pruned)
    } else {
        anyhow::bail!(
            "{} secret(s) could not be pruned: {}",
            errors.len(),
            errors.join("; ")
        )
    }
}

fn mark_gone(synced_secrets: &mut HashMap<String, ResourceSyncState>, secret_name: &str) {
    let state = synced_secrets
        .entry(secret_name.to_string())
        .or_insert(ResourceSyncState {
            exists: false,
            update_count: 0,
        });
    state.exists = false;
}

fn is_not_found(error: &anyhow::Error) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("404") || message.contains("not found") || message.contains("not_found")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synced(names: &[(&str, bool)]) -> HashMap<String, ResourceSyncState> {
        names
            .iter()
            .map(|(name, exists)| {
                (
                    (*name).to_string(),
                    ResourceSyncState {
                        exists: *exists,
                        update_count: 1,
                    },
                )
            })
            .collect()
    }

    fn desired(names: &[&str]) -> HashSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn config(prune_annotation: Option<&str>) -> SecretManagerConfig {
        let mut config: SecretManagerConfig = serde_json::from_value(serde_json::json!({
            "apiVersion": "secret-management.octopilot.io/v1beta1",
            "kind": "SecretManagerConfig",
            "metadata": { "name": "app", "namespace": "ns" },
            "spec": {
                "sourceRef": { "kind": "GitRepository", "name": "repo", "namespace": "flux-system" },
                "provider": { "gcp": { "projectId": "p", "location": "us-east4" } },
                "secrets": { "environment": "dev" }
            }
        }))
        .expect("a minimal SecretManagerConfig");
        if let Some(value) = prune_annotation {
            config.metadata.annotations = Some(std::collections::BTreeMap::from([(
                PRUNE_ANNOTATION.to_string(),
                value.to_string(),
            )]));
        }
        config
    }

    #[test]
    fn pruning_is_off_unless_the_annotation_says_true() {
        assert!(!prune_enabled(&config(None)), "off by default");
        for off in ["false", "", "yes", "1", "enabled"] {
            assert!(!prune_enabled(&config(Some(off))), "{off:?}");
        }
        for on in ["true", "True", " true "] {
            assert!(prune_enabled(&config(Some(on))), "{on:?}");
        }
    }

    #[test]
    fn a_secret_no_longer_in_the_files_is_stale() {
        let previous = synced(&[("app-A", true), ("app-B", true), ("app-C", true)]);
        assert_eq!(
            stale_secrets(&previous, &desired(&["app-A", "app-C"])),
            vec!["app-B".to_string()]
        );
    }

    #[test]
    fn nothing_is_stale_when_every_synced_secret_is_still_wanted() {
        let previous = synced(&[("app-A", true)]);
        assert!(stale_secrets(&previous, &desired(&["app-A", "app-NEW"])).is_empty());
    }

    /// Only what the status records as existing is a candidate: a secret already pruned,
    /// or one that was never created, is not deleted again.
    #[test]
    fn a_secret_already_gone_is_not_stale() {
        let previous = synced(&[("app-OLD", false)]);
        assert!(stale_secrets(&previous, &desired(&["app-A"])).is_empty());
    }

    #[test]
    fn stale_secrets_are_listed_in_order() {
        let previous = synced(&[("z", true), ("a", true), ("m", true)]);
        assert_eq!(
            stale_secrets(&previous, &desired(&["keep"])),
            vec!["a".to_string(), "m".to_string(), "z".to_string()]
        );
    }

    #[test]
    fn not_found_is_recognised_in_provider_errors() {
        assert!(is_not_found(&anyhow::anyhow!("HTTP 404: Secret not found")));
        assert!(is_not_found(&anyhow::anyhow!("status: NOT_FOUND")));
        assert!(!is_not_found(&anyhow::anyhow!(
            "HTTP 403: permission denied"
        )));
    }
}
