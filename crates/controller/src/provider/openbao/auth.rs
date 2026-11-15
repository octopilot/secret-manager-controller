//! # OpenBao Kubernetes Authentication
//!
//! Reads the controller's projected ServiceAccount JWT and exchanges it at the
//! OpenBao Kubernetes auth endpoint for a short-lived client token. No static
//! credentials are stored — this mirrors the Workload Identity / IRSA approach
//! used by the cloud providers.

use crate::crd::{OpenBaoAuthConfig, OpenBaoConfig};
use anyhow::{Context, Result};

/// Standard in-pod path of the projected ServiceAccount token.
const DEFAULT_SA_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";

/// Resolved Kubernetes auth parameters for an OpenBao client.
#[derive(Debug, Clone)]
pub struct KubernetesAuth {
    /// OpenBao Kubernetes auth role to authenticate against.
    pub role: String,
    /// Filesystem path of the ServiceAccount JWT to present.
    pub token_path: String,
}

impl KubernetesAuth {
    /// Build auth parameters from the CRD config.
    ///
    /// # Errors
    /// Returns an error if the config does not specify Kubernetes auth.
    pub fn from_config(config: &OpenBaoConfig) -> Result<Self> {
        match &config.auth {
            Some(OpenBaoAuthConfig::Kubernetes {
                role,
                service_account_token_path,
            }) => Ok(Self {
                role: role.clone(),
                token_path: service_account_token_path
                    .clone()
                    .unwrap_or_else(|| DEFAULT_SA_TOKEN_PATH.to_string()),
            }),
            None => Err(anyhow::anyhow!(
                "provider.openbao.auth is required (Kubernetes auth with a role)"
            )),
        }
    }

    /// Read the ServiceAccount JWT from disk.
    ///
    /// # Errors
    /// Returns an error if the token file cannot be read.
    pub fn read_jwt(&self) -> Result<String> {
        std::fs::read_to_string(&self.token_path)
            .map(|s| s.trim().to_string())
            .with_context(|| {
                format!(
                    "Failed to read ServiceAccount token at {} for OpenBao Kubernetes auth",
                    self.token_path
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_auth(auth: Option<OpenBaoAuthConfig>) -> OpenBaoConfig {
        OpenBaoConfig {
            address: "http://openbao.openbao.svc:8200".to_string(),
            mount: "secret".to_string(),
            namespace: None,
            tls_ca_secret_ref: None,
            auth,
        }
    }

    #[test]
    fn from_config_kubernetes_defaults_token_path() {
        let cfg = config_with_auth(Some(OpenBaoAuthConfig::Kubernetes {
            role: "secret-manager-controller".to_string(),
            service_account_token_path: None,
        }));
        let auth = KubernetesAuth::from_config(&cfg).expect("kubernetes auth");
        assert_eq!(auth.role, "secret-manager-controller");
        assert_eq!(auth.token_path, DEFAULT_SA_TOKEN_PATH);
    }

    #[test]
    fn from_config_honors_custom_token_path() {
        let cfg = config_with_auth(Some(OpenBaoAuthConfig::Kubernetes {
            role: "smc".to_string(),
            service_account_token_path: Some("/tmp/projected/token".to_string()),
        }));
        let auth = KubernetesAuth::from_config(&cfg).expect("kubernetes auth");
        assert_eq!(auth.token_path, "/tmp/projected/token");
    }

    #[test]
    fn from_config_missing_auth_is_error() {
        let cfg = config_with_auth(None);
        match KubernetesAuth::from_config(&cfg) {
            Ok(_) => panic!("expected an error when auth is missing"),
            Err(_) => {}
        }
    }

    #[test]
    fn read_jwt_reads_and_trims_whitespace() {
        let path = std::env::temp_dir().join(format!("openbao-test-jwt-{}", std::process::id()));
        std::fs::write(&path, "  my-jwt-token\n").expect("write temp jwt");
        let auth = KubernetesAuth {
            role: "smc".to_string(),
            token_path: path.to_string_lossy().to_string(),
        };
        let jwt = auth.read_jwt().expect("read jwt");
        assert_eq!(jwt, "my-jwt-token");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_jwt_missing_file_is_error() {
        let auth = KubernetesAuth {
            role: "smc".to_string(),
            token_path: "/nonexistent/openbao/token".to_string(),
        };
        match auth.read_jwt() {
            Ok(_) => panic!("expected an error for a missing token file"),
            Err(_) => {}
        }
    }
}
