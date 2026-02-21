//! Azure Key Vault and App Configuration API Paths
//!
//! This module defines all API paths for Azure services to ensure consistency
//! between the controller and mock server implementations.
//!
//! These paths are based on the official Azure Key Vault REST API documentation:
//! https://learn.microsoft.com/en-us/rest/api/keyvault/
//!
//! **IMPORTANT**: Azure Key Vault uses RESTful paths with api-version query parameter.
//! Some endpoints use trailing slashes, others don't - this is important for matching.

/// Azure Key Vault Secrets API paths
pub mod key_vault {
    /// Base path for Key Vault operations
    pub const BASE: &str = "/";

    /// List all secrets
    /// GET /secrets
    pub const LIST_SECRETS: &str = "/secrets";

    /// Get secret (with trailing slash)
    /// GET /secrets/{name}/
    pub fn get_secret(name: &str) -> String {
        format!("/secrets/{name}/")
    }

    /// Get specific secret version
    /// GET /secrets/{name}/{version}
    pub fn get_secret_version(name: &str, version: &str) -> String {
        format!("/secrets/{name}/{version}")
    }

    /// List secret versions
    /// GET /secrets/{name}/versions
    pub fn list_secret_versions(name: &str) -> String {
        format!("/secrets/{name}/versions")
    }

    /// Set secret (without trailing slash)
    /// PUT /secrets/{name}
    pub fn set_secret(name: &str) -> String {
        format!("/secrets/{name}")
    }

    /// Delete secret (all versions)
    /// DELETE /secrets/{name}
    pub fn delete_secret(name: &str) -> String {
        format!("/secrets/{name}")
    }

    /// Update secret attributes (enabled/disabled)
    /// PATCH /secrets/{name}
    pub fn update_secret(name: &str) -> String {
        format!("/secrets/{name}")
    }

    /// Backup secret
    /// POST /secrets/{name}/backup
    pub fn backup_secret(name: &str) -> String {
        format!("/secrets/{name}/backup")
    }

    /// Restore secret from backup
    /// POST /secrets/restore
    pub const RESTORE_SECRET: &str = "/secrets/restore";

    /// Get deleted secret
    /// GET /deletedsecrets/{name}
    pub fn get_deleted_secret(name: &str) -> String {
        format!("/deletedsecrets/{name}")
    }

    /// List deleted secrets
    /// GET /deletedsecrets
    pub const LIST_DELETED_SECRETS: &str = "/deletedsecrets";

    /// Recover deleted secret
    /// POST /deletedsecrets/{name}/recover
    pub fn recover_deleted_secret(name: &str) -> String {
        format!("/deletedsecrets/{name}/recover")
    }

    /// Purge deleted secret
    /// DELETE /deletedsecrets/{name}
    pub fn purge_deleted_secret(name: &str) -> String {
        format!("/deletedsecrets/{name}")
    }
}

/// Azure App Configuration API paths
pub mod app_configuration {
    /// Base path for App Configuration operations
    pub const BASE: &str = "/";

    /// Get key-value
    /// GET /kv/{key}
    pub fn get_key_value(key: &str) -> String {
        format!("/kv/{key}")
    }

    /// Set key-value
    /// PUT /kv/{key}
    pub fn set_key_value(key: &str) -> String {
        format!("/kv/{key}")
    }

    /// Delete key-value
    /// DELETE /kv/{key}
    pub fn delete_key_value(key: &str) -> String {
        format!("/kv/{key}")
    }

    /// List key-values
    /// GET /kv
    pub const LIST_KEY_VALUES: &str = "/kv";
}

/// Route constants for Axum routes
///
/// These constants are the single source of truth for Axum route patterns.
/// They are validated against PathBuilder output in tests to ensure consistency.
pub mod routes {
    /// Azure Key Vault Secrets route patterns
    pub mod key_vault {
        /// GET /secrets - List all secrets
        pub const LIST_SECRETS: &str = "/secrets";

        /// GET /secrets/{name}/ - Get secret (with trailing slash)
        pub const GET_SECRET: &str = "/secrets/{name}/";

        /// GET /secrets/{name}/{version} - Get specific secret version
        pub const GET_SECRET_VERSION: &str = "/secrets/{name}/{version}";

        /// GET /secrets/{name}/versions - List secret versions
        pub const LIST_SECRET_VERSIONS: &str = "/secrets/{name}/versions";

        /// PUT /secrets/{name} - Set secret (without trailing slash)
        pub const SET_SECRET: &str = "/secrets/{name}";

        /// DELETE /secrets/{name} - Delete secret
        pub const DELETE_SECRET: &str = "/secrets/{name}";

        /// PATCH /secrets/{name} - Update secret attributes
        pub const UPDATE_SECRET: &str = "/secrets/{name}";

        /// POST /secrets/{name}/backup - Backup secret
        pub const BACKUP_SECRET: &str = "/secrets/{name}/backup";

        /// POST /secrets/restore - Restore secret from backup
        pub const RESTORE_SECRET: &str = "/secrets/restore";

        /// GET /deletedsecrets/{name} - Get deleted secret
        pub const GET_DELETED_SECRET: &str = "/deletedsecrets/{name}";

        /// GET /deletedsecrets - List deleted secrets
        pub const LIST_DELETED_SECRETS: &str = "/deletedsecrets";

        /// POST /deletedsecrets/{name}/recover - Recover deleted secret
        pub const RECOVER_DELETED_SECRET: &str = "/deletedsecrets/{name}/recover";

        /// DELETE /deletedsecrets/{name} - Purge deleted secret
        pub const PURGE_DELETED_SECRET: &str = "/deletedsecrets/{name}";
    }

    /// Azure App Configuration route patterns
    pub mod app_configuration {
        /// GET /kv - List key-values
        pub const LIST_KEY_VALUES: &str = "/kv";

        /// GET /kv/{key} - Get key-value
        pub const GET_KEY_VALUE: &str = "/kv/{key}";

        /// PUT /kv/{key} - Set key-value
        pub const SET_KEY_VALUE: &str = "/kv/{key}";

        /// DELETE /kv/{key} - Delete key-value
        pub const DELETE_KEY_VALUE: &str = "/kv/{key}";
    }
}
