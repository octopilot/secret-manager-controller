//! # OpenBao HTTP Client
//!
//! Native REST client for the OpenBao KV v2 secrets engine over `reqwest` +
//! rustls (no OpenSSL), modelled on the GCP REST provider. Handles Kubernetes
//! auth login, token caching, transparent re-login on `403`, and the KV v2
//! read/write/delete/undelete operations.

use anyhow::{Context, Result};
use reqwest::{Client, Method, StatusCode};
use tokio::sync::RwLock;
use tracing::{info, warn};
use zeroize::Zeroizing;

use super::auth::KubernetesAuth;
use super::types::{KvMetadataResponse, KvReadResponse, LoginResponse};
use crate::crd::OpenBaoConfig;

/// OpenBao KV v2 client.
pub struct OpenBaoClient {
    http: Client,
    /// Base URL without a trailing slash, e.g. `https://openbao.openbao.svc:8200`.
    base_url: String,
    /// KV v2 mount name (e.g. `secret`).
    mount: String,
    /// Optional OpenBao namespace (`X-Vault-Namespace`).
    namespace: Option<String>,
    /// Kubernetes auth parameters used to (re)login.
    auth: KubernetesAuth,
    /// Cached client token; zeroized on overwrite and drop.
    token: RwLock<Zeroizing<String>>,
}

impl std::fmt::Debug for OpenBaoClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenBaoClient")
            .field("base_url", &self.base_url)
            .field("mount", &self.mount)
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

impl OpenBaoClient {
    /// Construct a client and perform the initial Kubernetes auth login.
    ///
    /// # Errors
    /// Returns an error if auth config is missing, the HTTP client cannot be
    /// built, or the initial login fails.
    pub async fn new(config: &OpenBaoConfig) -> Result<Self> {
        let auth = KubernetesAuth::from_config(config)?;
        // rustls is enforced workspace-wide via the reqwest feature set.
        let http = Client::builder()
            .build()
            .context("Failed to build OpenBao HTTP client")?;

        // TODO(OB-8): when config.tls_ca_secret_ref is set, load the CA from the
        // referenced Kubernetes Secret and build a custom rustls root store.

        let client = Self {
            http,
            base_url: config.address.trim_end_matches('/').to_string(),
            mount: config.mount.clone(),
            namespace: config.namespace.clone(),
            auth,
            token: RwLock::new(Zeroizing::new(String::new())),
        };
        client.login().await?;
        Ok(client)
    }

    /// Exchange the ServiceAccount JWT for a client token and cache it.
    async fn login(&self) -> Result<()> {
        // Zeroizing wipes the JWT buffer on drop.
        let jwt = Zeroizing::new(self.auth.read_jwt()?);
        let url = format!("{}/v1/auth/kubernetes/login", self.base_url);
        let body = serde_json::json!({ "role": self.auth.role, "jwt": jwt.as_str() });

        let mut req = self.http.post(&url).json(&body);
        if let Some(ns) = &self.namespace {
            req = req.header("X-Vault-Namespace", ns);
        }
        let resp = req.send().await.context("OpenBao login request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "OpenBao Kubernetes login failed (HTTP {}): {}",
                status.as_u16(),
                text
            ));
        }

        let parsed: LoginResponse = resp
            .json()
            .await
            .context("Failed to parse OpenBao login response")?;
        {
            let mut guard = self.token.write().await;
            *guard = Zeroizing::new(parsed.auth.client_token);
        }
        info!(
            renewable = parsed.auth.renewable,
            lease_seconds = parsed.auth.lease_duration,
            "Authenticated to OpenBao via Kubernetes auth"
        );
        Ok(())
    }

    /// Snapshot the current token as an owned string for a single request.
    async fn token(&self) -> String {
        self.token.read().await.as_str().to_string()
    }

    /// Send one authenticated request (no retry).
    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<reqwest::Response> {
        let url = format!("{}/v1/{}", self.base_url, path.trim_start_matches('/'));
        let token = self.token().await;
        let mut req = self
            .http
            .request(method, &url)
            .header("X-Vault-Token", token);
        if let Some(ns) = &self.namespace {
            req = req.header("X-Vault-Namespace", ns);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        req.send().await.context("OpenBao request failed")
    }

    /// Send an authenticated request, re-logging in once on `403`.
    async fn authed(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response> {
        let resp = self.send(method.clone(), path, body.as_ref()).await?;
        if resp.status() == StatusCode::FORBIDDEN {
            warn!("OpenBao returned 403 Forbidden; re-authenticating and retrying once");
            self.login().await?;
            return self.send(method, path, body.as_ref()).await;
        }
        Ok(resp)
    }

    /// Read the latest value of a secret. Returns `None` for a missing or
    /// soft-deleted-latest secret (both surface as HTTP 404).
    ///
    /// # Errors
    /// Returns an error on a sealed/unavailable server or an unexpected status.
    pub async fn kv_get(&self, name: &str) -> Result<Option<String>> {
        let path = format!("{}/data/{}", self.mount, name);
        let resp = self.authed(Method::GET, &path, None).await?;
        match resp.status() {
            StatusCode::OK => {
                let parsed: KvReadResponse = resp
                    .json()
                    .await
                    .context("Failed to parse OpenBao read response")?;
                Ok(parsed
                    .data
                    .data
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    .map(std::string::ToString::to_string))
            }
            StatusCode::NOT_FOUND => Ok(None),
            status => Err(Self::api_error(status, resp, "read").await),
        }
    }

    /// Write (create a new version of) a secret with a single `value` field.
    ///
    /// # Errors
    /// Returns an error on a non-2xx response.
    pub async fn kv_put(&self, name: &str, value: &str) -> Result<()> {
        let path = format!("{}/data/{}", self.mount, name);
        let body = serde_json::json!({ "data": { "value": value } });
        let resp = self.authed(Method::POST, &path, Some(body)).await?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(Self::api_error(status, resp, "write").await)
        }
    }

    /// Delete a secret and all its versions (irreversible).
    ///
    /// # Errors
    /// Returns an error on a non-2xx, non-404 response.
    pub async fn kv_delete_all(&self, name: &str) -> Result<()> {
        let path = format!("{}/metadata/{}", self.mount, name);
        let resp = self.authed(Method::DELETE, &path, None).await?;
        let status = resp.status();
        if status.is_success() || status == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(Self::api_error(status, resp, "delete").await)
        }
    }

    /// Soft-delete the latest version (reversible). Returns `false` if the
    /// secret is absent or already deleted.
    ///
    /// # Errors
    /// Returns an error on a non-2xx response.
    pub async fn kv_soft_delete_latest(&self, name: &str) -> Result<bool> {
        if self.kv_get(name).await?.is_none() {
            return Ok(false);
        }
        let path = format!("{}/data/{}", self.mount, name);
        let resp = self.authed(Method::DELETE, &path, None).await?;
        let status = resp.status();
        if status.is_success() {
            Ok(true)
        } else {
            Err(Self::api_error(status, resp, "disable").await)
        }
    }

    /// Undelete the latest version (reverse of [`kv_soft_delete_latest`]).
    /// Returns `false` if the secret is already readable or has no versions.
    ///
    /// # Errors
    /// Returns an error on a non-2xx response.
    pub async fn kv_undelete_latest(&self, name: &str) -> Result<bool> {
        if self.kv_get(name).await?.is_some() {
            return Ok(false);
        }
        let Some(version) = self.current_version(name).await? else {
            return Ok(false);
        };
        let path = format!("{}/undelete/{}", self.mount, name);
        let body = serde_json::json!({ "versions": [version] });
        let resp = self.authed(Method::POST, &path, Some(body)).await?;
        let status = resp.status();
        if status.is_success() {
            Ok(true)
        } else {
            Err(Self::api_error(status, resp, "enable").await)
        }
    }

    /// Latest version number from KV v2 metadata, or `None` if the secret has
    /// no metadata.
    async fn current_version(&self, name: &str) -> Result<Option<u64>> {
        let path = format!("{}/metadata/{}", self.mount, name);
        let resp = self.authed(Method::GET, &path, None).await?;
        match resp.status() {
            StatusCode::OK => {
                let parsed: KvMetadataResponse = resp
                    .json()
                    .await
                    .context("Failed to parse OpenBao metadata response")?;
                Ok(Some(parsed.data.current_version))
            }
            StatusCode::NOT_FOUND => Ok(None),
            status => Err(Self::api_error(status, resp, "metadata").await),
        }
    }

    /// Build a contextual error from a failed response. A `503` means the server
    /// is sealed or on standby — a retryable condition the reconciler's error
    /// policy will requeue with backoff.
    async fn api_error(status: StatusCode, resp: reqwest::Response, op: &str) -> anyhow::Error {
        let text = resp.text().await.unwrap_or_default();
        if status == StatusCode::SERVICE_UNAVAILABLE {
            // TODO(OB-5): surface a Ready=False / OpenBaoUnavailable status condition.
            return anyhow::anyhow!(
                "OpenBao is sealed or unavailable during {} (HTTP 503): {}",
                op,
                text
            );
        }
        anyhow::anyhow!("OpenBao {} failed (HTTP {}): {}", op, status.as_u16(), text)
    }
}
