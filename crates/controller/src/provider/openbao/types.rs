//! # OpenBao API Types
//!
//! Request/response DTOs for the OpenBao (Vault-compatible) HTTP API:
//! Kubernetes auth login and the KV v2 secrets engine.

use serde::Deserialize;

/// Response from `POST /v1/auth/kubernetes/login`.
#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    /// Authentication payload containing the client token.
    pub auth: LoginAuth,
}

/// The `auth` object of a successful login.
#[derive(Debug, Deserialize)]
pub struct LoginAuth {
    /// Short-lived client token used as `X-Vault-Token` on subsequent requests.
    pub client_token: String,
    /// Token lease duration in seconds.
    #[serde(default)]
    pub lease_duration: u64,
    /// Whether the token is renewable.
    #[serde(default)]
    pub renewable: bool,
}

/// Response from `GET /v1/<mount>/data/<name>` (KV v2 read).
#[derive(Debug, Deserialize)]
pub struct KvReadResponse {
    /// Outer `data` wrapper.
    pub data: KvReadData,
}

/// Inner `data` of a KV v2 read (`data.data` holds the secret fields).
#[derive(Debug, Deserialize)]
pub struct KvReadData {
    /// The secret's key/value fields. Kept as a generic JSON value so
    /// manually-created secrets with non-string fields don't fail deserialization.
    pub data: serde_json::Value,
}

/// Response from `GET /v1/<mount>/metadata/<name>` (KV v2 version metadata).
#[derive(Debug, Deserialize)]
pub struct KvMetadataResponse {
    /// Outer `data` wrapper.
    pub data: KvMetadata,
}

/// Version metadata for a secret.
#[derive(Debug, Deserialize)]
pub struct KvMetadata {
    /// The latest version number.
    pub current_version: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_response_parses_all_fields() {
        let json = r#"{"auth":{"client_token":"s.abc","lease_duration":3600,"renewable":true}}"#;
        let parsed: LoginResponse = serde_json::from_str(json).expect("login parse");
        assert_eq!(parsed.auth.client_token, "s.abc");
        assert_eq!(parsed.auth.lease_duration, 3600);
        assert!(parsed.auth.renewable);
    }

    #[test]
    fn login_response_defaults_optional_fields() {
        // lease_duration / renewable are #[serde(default)].
        let json = r#"{"auth":{"client_token":"s.abc"}}"#;
        let parsed: LoginResponse = serde_json::from_str(json).expect("login parse");
        assert_eq!(parsed.auth.lease_duration, 0);
        assert!(!parsed.auth.renewable);
    }

    #[test]
    fn kv_read_extracts_value_and_ignores_metadata() {
        let json = r#"{"data":{"data":{"value":"s3cr3t"},"metadata":{"version":1}}}"#;
        let parsed: KvReadResponse = serde_json::from_str(json).expect("read parse");
        assert_eq!(
            parsed
                .data
                .data
                .get("value")
                .and_then(serde_json::Value::as_str),
            Some("s3cr3t")
        );
    }

    #[test]
    fn kv_metadata_parses_current_version() {
        let json = r#"{"data":{"current_version":7,"oldest_version":1}}"#;
        let parsed: KvMetadataResponse = serde_json::from_str(json).expect("metadata parse");
        assert_eq!(parsed.data.current_version, 7);
    }
}
