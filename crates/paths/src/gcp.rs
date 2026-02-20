//! GCP Secret Manager and Parameter Manager API Paths
//!
//! This module defines all API paths as constants to ensure consistency
//! between the controller and mock server implementations.
//!
//! These paths are based on the official GCP Secret Manager REST API v1 documentation:
//! https://cloud.google.com/secret-manager/docs/reference/rest
//!
//! **IMPORTANT**: The mock server paths are the source of truth as they were
//! built directly from the GCP documentation.

/// GCP Secret Manager API paths
pub mod secret_manager {
    /// Base path for Secret Manager operations
    pub const BASE: &str = "/v1/projects/{project}/secrets";

    /// Create a new secret
    /// POST /v1/projects/{project}/secrets
    pub fn create_secret(project: &str) -> String {
        format!("/v1/projects/{project}/secrets")
    }

    /// List secrets in a project
    /// GET /v1/projects/{project}/secrets
    pub fn list_secrets(project: &str) -> String {
        format!("/v1/projects/{project}/secrets")
    }

    /// Get secret metadata
    /// GET /v1/projects/{project}/secrets/{secret}
    pub fn get_secret_metadata(project: &str, secret: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}")
    }

    /// Update secret metadata
    /// PATCH /v1/projects/{project}/secrets/{secret}
    pub fn update_secret_metadata(project: &str, secret: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}")
    }

    /// Delete a secret
    /// DELETE /v1/projects/{project}/secrets/{secret}
    pub fn delete_secret(project: &str, secret: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}")
    }

    /// Add a new version to a secret
    /// POST /v1/projects/{project}/secrets/{secret}:addVersion
    pub fn add_version(project: &str, secret: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}:addVersion")
    }

    /// Enable a secret
    /// POST /v1/projects/{project}/secrets/{secret}:enable
    pub fn enable_secret(project: &str, secret: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}:enable")
    }

    /// Disable a secret
    /// POST /v1/projects/{project}/secrets/{secret}:disable
    pub fn disable_secret(project: &str, secret: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}:disable")
    }

    /// List versions of a secret
    /// GET /v1/projects/{project}/secrets/{secret}/versions
    pub fn list_versions(project: &str, secret: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}/versions")
    }

    /// Get a specific version of a secret
    /// GET /v1/projects/{project}/secrets/{secret}/versions/{version}
    pub fn get_version(project: &str, secret: &str, version: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}/versions/{version}")
    }

    /// Access latest version (get secret value)
    /// GET /v1/projects/{project}/secrets/{secret}/versions/latest:access
    pub fn access_latest_version(project: &str, secret: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}/versions/latest:access")
    }

    /// Access specific version (get secret value)
    /// GET /v1/projects/{project}/secrets/{secret}/versions/{version}:access
    pub fn access_version(project: &str, secret: &str, version: &str) -> String {
        format!("/v1/projects/{project}/secrets/{secret}/versions/{version}:access")
    }

    /// Path template for controller use (without /v1 prefix, as controller adds it)
    /// Returns: projects/{project}/secrets/{secret}
    pub fn secret_path(project: &str, secret: &str) -> String {
        format!("projects/{project}/secrets/{secret}")
    }

    /// Path template for controller use (without /v1 prefix)
    /// Returns: projects/{project}/secrets
    pub fn secrets_base(project: &str) -> String {
        format!("projects/{project}/secrets")
    }

    /// Path template for controller use (without /v1 prefix)
    /// Returns: projects/{project}/secrets/{secret}:addVersion
    pub fn add_version_path(project: &str, secret: &str) -> String {
        format!("projects/{project}/secrets/{secret}:addVersion")
    }

    /// Path template for controller use (without /v1 prefix)
    /// Returns: projects/{project}/secrets/{secret}/versions/latest:access
    pub fn access_latest_version_path(project: &str, secret: &str) -> String {
        format!("projects/{project}/secrets/{secret}/versions/latest:access")
    }

    /// Path template for controller use (without /v1 prefix)
    /// Returns: projects/{project}/secrets/{secret}:enable
    pub fn enable_secret_path(project: &str, secret: &str) -> String {
        format!("projects/{project}/secrets/{secret}:enable")
    }

    /// Path template for controller use (without /v1 prefix)
    /// Returns: projects/{project}/secrets/{secret}:disable
    pub fn disable_secret_path(project: &str, secret: &str) -> String {
        format!("projects/{project}/secrets/{secret}:disable")
    }
}

/// GCP Parameter Manager API paths
pub mod parameter_manager {
    /// Create a new parameter
    /// POST /v1/projects/{project}/locations/{location}/parameters
    pub fn create_parameter(project: &str, location: &str) -> String {
        format!("/v1/projects/{project}/locations/{location}/parameters")
    }

    /// List parameters
    /// GET /v1/projects/{project}/locations/{location}/parameters
    pub fn list_parameters(project: &str, location: &str) -> String {
        format!("/v1/projects/{project}/locations/{location}/parameters")
    }

    /// Get parameter
    /// GET /v1/projects/{project}/locations/{location}/parameters/{parameter}
    pub fn get_parameter(project: &str, location: &str, parameter: &str) -> String {
        format!("/v1/projects/{project}/locations/{location}/parameters/{parameter}")
    }

    /// Update parameter
    /// PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter}
    pub fn update_parameter(project: &str, location: &str, parameter: &str) -> String {
        format!("/v1/projects/{project}/locations/{location}/parameters/{parameter}")
    }

    /// Delete parameter
    /// DELETE /v1/projects/{project}/locations/{location}/parameters/{parameter}
    pub fn delete_parameter(project: &str, location: &str, parameter: &str) -> String {
        format!("/v1/projects/{project}/locations/{location}/parameters/{parameter}")
    }

    /// Create parameter version
    /// POST /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions
    pub fn create_version(project: &str, location: &str, parameter: &str) -> String {
        format!("/v1/projects/{project}/locations/{location}/parameters/{parameter}/versions")
    }

    /// List parameter versions
    /// GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions
    pub fn list_versions(project: &str, location: &str, parameter: &str) -> String {
        format!("/v1/projects/{project}/locations/{location}/parameters/{parameter}/versions")
    }

    /// Get parameter version
    /// GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}
    pub fn get_version(project: &str, location: &str, parameter: &str, version: &str) -> String {
        format!(
            "/v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}"
        )
    }

    /// Update parameter version
    /// PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}
    pub fn update_version(project: &str, location: &str, parameter: &str, version: &str) -> String {
        format!(
            "/v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}"
        )
    }

    /// Delete parameter version
    /// DELETE /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}
    pub fn delete_version(project: &str, location: &str, parameter: &str, version: &str) -> String {
        format!(
            "/v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}"
        )
    }

    /// Render parameter version
    /// GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}:render
    pub fn render_version(project: &str, location: &str, parameter: &str, version: &str) -> String {
        format!(
            "/v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}:render"
        )
    }

    /// Get location
    /// GET /v1/projects/{project}/locations/{location}
    pub fn get_location(project: &str, location: &str) -> String {
        format!("/v1/projects/{project}/locations/{location}")
    }

    /// List locations
    /// GET /v1/projects/{project}/locations
    pub fn list_locations(project: &str) -> String {
        format!("/v1/projects/{project}/locations")
    }
}

/// Route constants for Axum routes
///
/// These constants are the single source of truth for Axum route patterns.
/// They are validated against PathBuilder output in tests to ensure consistency.
pub mod routes {
    /// GCP Secret Manager route patterns
    pub mod secret_manager {
        /// POST /v1/projects/{project}/secrets - Create secret
        /// GET /v1/projects/{project}/secrets - List secrets
        pub const CREATE_SECRET: &str = "/v1/projects/{project}/secrets";

        /// GET /v1/projects/{project}/secrets/{secret} - Get secret metadata
        /// PATCH /v1/projects/{project}/secrets/{secret} - Update secret metadata
        /// DELETE /v1/projects/{project}/secrets/{secret} - Delete secret
        pub const SECRET: &str = "/v1/projects/{project}/secrets/{secret}";

        /// GET /v1/projects/{project}/secrets/{secret}/versions - List secret versions
        pub const SECRET_VERSIONS: &str = "/v1/projects/{project}/secrets/{secret}/versions";

        /// GET /v1/projects/{project}/secrets/{secret}/versions/{version} - Get specific version
        pub const SECRET_VERSION: &str =
            "/v1/projects/{project}/secrets/{secret}/versions/{version}";
    }

    /// GCP Parameter Manager route patterns
    pub mod parameter_manager {
        /// POST /v1/projects/{project}/locations/{location}/parameters - Create parameter
        /// GET /v1/projects/{project}/locations/{location}/parameters - List parameters
        pub const CREATE_PARAMETER: &str = "/v1/projects/{project}/locations/{location}/parameters";

        /// GET /v1/projects/{project}/locations/{location}/parameters/{parameter} - Get parameter
        /// PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter} - Update parameter
        /// DELETE /v1/projects/{project}/locations/{location}/parameters/{parameter} - Delete parameter
        pub const PARAMETER: &str =
            "/v1/projects/{project}/locations/{location}/parameters/{parameter}";

        /// POST /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions - Create parameter version
        /// GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions - List parameter versions
        pub const PARAMETER_VERSIONS: &str =
            "/v1/projects/{project}/locations/{location}/parameters/{parameter}/versions";

        /// GET /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Get specific parameter version
        /// PATCH /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Update parameter version
        /// DELETE /v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version} - Delete parameter version
        pub const PARAMETER_VERSION: &str =
            "/v1/projects/{project}/locations/{location}/parameters/{parameter}/versions/{version}";
    }

    /// GCP Location route patterns
    pub mod locations {
        /// GET /v1/projects/{project}/locations/{location} - Get location
        pub const LOCATION: &str = "/v1/projects/{project}/locations/{location}";

        /// GET /v1/projects/{project}/locations - List locations
        pub const LIST_LOCATIONS: &str = "/v1/projects/{project}/locations";
    }
}

#[cfg(test)]
mod route_validation {
    use super::routes;
    use crate::builder::PathBuilder;
    use crate::operations::GcpOperation;

    #[test]
    fn validate_secret_manager_routes() {
        // Validate CREATE_SECRET route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::CreateSecret)
            .project("test-project")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::secret_manager::CREATE_SECRET);

        // Validate SECRET route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::GetSecret)
            .project("test-project")
            .secret("test-secret")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::secret_manager::SECRET);

        // Validate SECRET_VERSIONS route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::ListVersions)
            .project("test-project")
            .secret("test-secret")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::secret_manager::SECRET_VERSIONS);

        // Validate SECRET_VERSION route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::GetVersion)
            .project("test-project")
            .secret("test-secret")
            .version("123")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::secret_manager::SECRET_VERSION);
    }

    #[test]
    fn validate_parameter_manager_routes() {
        // Validate CREATE_PARAMETER route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::CreateParameter)
            .project("test-project")
            .location("us-central1")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::parameter_manager::CREATE_PARAMETER);

        // Validate PARAMETER route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::GetParameter)
            .project("test-project")
            .location("us-central1")
            .parameter("test-parameter")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::parameter_manager::PARAMETER);

        // Validate PARAMETER_VERSIONS route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::ListParameterVersions)
            .project("test-project")
            .location("us-central1")
            .parameter("test-parameter")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::parameter_manager::PARAMETER_VERSIONS);

        // Validate PARAMETER_VERSION route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::GetParameterVersion)
            .project("test-project")
            .location("us-central1")
            .parameter("test-parameter")
            .version("123")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::parameter_manager::PARAMETER_VERSION);
    }

    #[test]
    fn validate_location_routes() {
        // Validate LOCATION route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::GetLocation)
            .project("test-project")
            .location("us-central1")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::locations::LOCATION);

        // Validate LIST_LOCATIONS route
        let route = PathBuilder::new()
            .gcp_operation(GcpOperation::ListLocations)
            .project("test-project")
            .build_route()
            .unwrap();
        assert_eq!(route, routes::locations::LIST_LOCATIONS);
    }
}
