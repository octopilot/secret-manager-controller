//! # OpenBao Provider
//!
//! In-cluster [OpenBao](https://openbao.org) provider implementing
//! [`SecretManagerProvider`] over the KV v2 secrets engine.
//!
//! - **Auth:** Kubernetes auth method (ServiceAccount JWT → short-lived token).
//! - **Secrets:** stored as `{"value": "<secret>"}` under `<mount>/data/<name>`.
//! - **disable/enable:** KV v2 version soft-delete / undelete (reversible).
//!
//! `location` is not applicable to a single-endpoint OpenBao and is ignored.

mod auth;
mod client;
mod types;

use crate::crd::OpenBaoConfig;
use crate::provider::SecretManagerProvider;
use crate::provider::common::{
    SecretComparison, compare_secret_value, log_secret_operation, record_secret_metrics,
};
use anyhow::Result;
use async_trait::async_trait;
use std::time::Instant;

use client::OpenBaoClient;

/// Provider name used for metrics and logging labels.
const PROVIDER_NAME: &str = "openbao";

/// OpenBao secret manager provider.
#[derive(Debug)]
pub struct OpenBaoProvider {
    client: OpenBaoClient,
}

/// Create an OpenBao provider, performing the initial Kubernetes auth login.
///
/// The `k8s_client` is currently unused but retained for parity with the other
/// providers and for the future custom-CA lookup (OB-8).
///
/// # Errors
/// Returns an error if the client cannot authenticate or connect.
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub async fn create_openbao_provider(
    config: &OpenBaoConfig,
    _k8s_client: &kube::Client,
) -> Result<Box<dyn SecretManagerProvider>> {
    let client = OpenBaoClient::new(config).await?;
    Ok(Box::new(OpenBaoProvider { client }))
}

#[async_trait]
impl SecretManagerProvider for OpenBaoProvider {
    async fn create_or_update_secret(
        &self,
        secret_name: &str,
        secret_value: &str,
        _environment: &str,
        _location: &str,
    ) -> Result<bool> {
        let start = Instant::now();
        let comparison =
            compare_secret_value(|| self.client.kv_get(secret_name), secret_value).await?;
        log_secret_operation(PROVIDER_NAME, secret_name, comparison);
        if comparison == SecretComparison::Unchanged {
            record_secret_metrics(PROVIDER_NAME, "no_change", start);
            return Ok(false);
        }
        self.client.kv_put(secret_name, secret_value).await?;
        record_secret_metrics(PROVIDER_NAME, "update", start);
        Ok(true)
    }

    async fn get_secret_value(&self, secret_name: &str) -> Result<Option<String>> {
        self.client.kv_get(secret_name).await
    }

    async fn delete_secret(&self, secret_name: &str) -> Result<()> {
        self.client.kv_delete_all(secret_name).await
    }

    async fn disable_secret(&self, secret_name: &str) -> Result<bool> {
        self.client.kv_soft_delete_latest(secret_name).await
    }

    async fn enable_secret(&self, secret_name: &str) -> Result<bool> {
        self.client.kv_undelete_latest(secret_name).await
    }
}
