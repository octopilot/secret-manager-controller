//! # SOPS keys by namespace
//!
//! The controller holds one SOPS private key per namespace that has a key Secret. A
//! SecretManagerConfig decrypts with the key of its own namespace, and with the
//! controller namespace's key when its own namespace has none. A key Secret created in
//! some other namespace never changes what a config decrypts with.

use std::collections::HashMap;

/// The namespace the controller runs in (`POD_NAMESPACE`).
#[must_use]
pub fn controller_namespace() -> String {
    std::env::var("POD_NAMESPACE").unwrap_or_else(|_| "octopilot-system".to_string())
}

/// SOPS private keys (age identities or armored GPG keys), by namespace.
#[derive(Default)]
pub struct SopsKeyStore {
    keys: HashMap<String, String>,
}

impl SopsKeyStore {
    /// A store holding the controller namespace's key, if there is one.
    #[must_use]
    pub fn with_controller_key(controller_namespace: &str, key: Option<String>) -> Self {
        let mut store = Self::default();
        if let Some(key) = key {
            store.set(controller_namespace, key);
        }
        store
    }

    /// Set the key of `namespace`.
    pub fn set(&mut self, namespace: &str, key: String) {
        self.keys.insert(namespace.to_string(), key);
    }

    /// Forget the key of `namespace`. True if it had one.
    pub fn remove(&mut self, namespace: &str) -> bool {
        self.keys.remove(namespace).is_some()
    }

    /// Whether `namespace` has a key of its own.
    #[must_use]
    pub fn has_key(&self, namespace: &str) -> bool {
        self.keys.contains_key(namespace)
    }

    /// The key a resource in `namespace` decrypts with: its namespace's own, else the
    /// controller namespace's.
    #[must_use]
    pub fn key_for(&self, namespace: &str, controller_namespace: &str) -> Option<&str> {
        self.keys
            .get(namespace)
            .or_else(|| self.keys.get(controller_namespace))
            .map(String::as_str)
    }
}

impl std::fmt::Debug for SopsKeyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut namespaces: Vec<&String> = self.keys.keys().collect();
        namespaces.sort();
        f.debug_struct("SopsKeyStore")
            .field("namespaces", &namespaces)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::SopsKeyStore;

    const CONTROLLER: &str = "secret-manager-controller";

    #[test]
    fn a_namespace_uses_its_own_key() {
        let mut store = SopsKeyStore::with_controller_key(CONTROLLER, Some("controller".into()));
        store.set("team-a", "a".into());
        assert_eq!(store.key_for("team-a", CONTROLLER), Some("a"));
        assert_eq!(store.key_for(CONTROLLER, CONTROLLER), Some("controller"));
    }

    #[test]
    fn a_namespace_without_a_key_uses_the_controller_key() {
        let store = SopsKeyStore::with_controller_key(CONTROLLER, Some("controller".into()));
        assert_eq!(store.key_for("team-b", CONTROLLER), Some("controller"));
    }

    /// The fault this replaces: one key for the whole controller, replaced by a key Secret
    /// appearing in any namespace.
    #[test]
    fn a_key_in_one_namespace_changes_nothing_for_the_others() {
        let mut store = SopsKeyStore::with_controller_key(CONTROLLER, Some("controller".into()));
        store.set("intruder", "other".into());
        assert_eq!(store.key_for(CONTROLLER, CONTROLLER), Some("controller"));
        assert_eq!(store.key_for("team-b", CONTROLLER), Some("controller"));
        assert_eq!(store.key_for("intruder", CONTROLLER), Some("other"));
    }

    #[test]
    fn removing_a_namespace_key_falls_back_to_the_controller_key() {
        let mut store = SopsKeyStore::with_controller_key(CONTROLLER, Some("controller".into()));
        store.set("team-a", "a".into());
        assert!(store.remove("team-a"));
        assert!(!store.remove("team-a"));
        assert_eq!(store.key_for("team-a", CONTROLLER), Some("controller"));
    }

    #[test]
    fn no_key_anywhere_is_no_key() {
        let store = SopsKeyStore::with_controller_key(CONTROLLER, None);
        assert_eq!(store.key_for("team-a", CONTROLLER), None);
        assert!(!store.has_key(CONTROLLER));
    }

    #[test]
    fn debug_output_names_namespaces_not_keys() {
        let mut store = SopsKeyStore::default();
        store.set("team-a", "AGE-SECRET-KEY-1SECRET".into());
        let printed = format!("{store:?}");
        assert!(printed.contains("team-a"));
        assert!(!printed.contains("SECRET-KEY"));
    }
}
