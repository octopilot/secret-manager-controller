//! AWS Secrets Manager and Parameter Store API Paths
//!
//! This module defines all API paths for AWS services to ensure consistency
//! between the controller and mock server implementations.
//!
//! **IMPORTANT**: AWS Secrets Manager uses a single POST endpoint "/" with
//! x-amz-target header to specify the operation. This module provides the
//! header values and operation names.

/// AWS Secrets Manager API operations
///
/// AWS Secrets Manager uses a single POST endpoint "/" with x-amz-target header.
/// All operations are sent to the same path with different header values.
pub mod secrets_manager {
    /// Base endpoint for all AWS Secrets Manager operations
    /// All requests are POST to this path with x-amz-target header
    pub const BASE: &str = "/";

    /// Create secret operation
    /// x-amz-target: secretsmanager.CreateSecret
    pub const CREATE_SECRET: &str = "secretsmanager.CreateSecret";

    /// Get secret value operation
    /// x-amz-target: secretsmanager.GetSecretValue
    pub const GET_SECRET_VALUE: &str = "secretsmanager.GetSecretValue";

    /// Describe secret operation
    /// x-amz-target: secretsmanager.DescribeSecret
    pub const DESCRIBE_SECRET: &str = "secretsmanager.DescribeSecret";

    /// Put secret value operation
    /// x-amz-target: secretsmanager.PutSecretValue
    pub const PUT_SECRET_VALUE: &str = "secretsmanager.PutSecretValue";

    /// Delete secret operation
    /// x-amz-target: secretsmanager.DeleteSecret
    pub const DELETE_SECRET: &str = "secretsmanager.DeleteSecret";

    /// Restore secret operation
    /// x-amz-target: secretsmanager.RestoreSecret
    pub const RESTORE_SECRET: &str = "secretsmanager.RestoreSecret";

    /// List secret versions operation
    /// x-amz-target: secretsmanager.ListSecretVersionIds
    pub const LIST_SECRET_VERSIONS: &str = "secretsmanager.ListSecretVersionIds";

    /// Update secret operation
    /// x-amz-target: secretsmanager.UpdateSecret
    pub const UPDATE_SECRET: &str = "secretsmanager.UpdateSecret";

    /// List secrets operation
    /// x-amz-target: secretsmanager.ListSecrets
    pub const LIST_SECRETS: &str = "secretsmanager.ListSecrets";

    /// Update secret version stage operation
    /// x-amz-target: secretsmanager.UpdateSecretVersionStage
    pub const UPDATE_SECRET_VERSION_STAGE: &str = "secretsmanager.UpdateSecretVersionStage";

    /// Tag resource operation
    /// x-amz-target: secretsmanager.TagResource
    pub const TAG_RESOURCE: &str = "secretsmanager.TagResource";

    /// Untag resource operation
    /// x-amz-target: secretsmanager.UntagResource
    pub const UNTAG_RESOURCE: &str = "secretsmanager.UntagResource";

    /// Get resource policy operation
    /// x-amz-target: secretsmanager.GetResourcePolicy
    pub const GET_RESOURCE_POLICY: &str = "secretsmanager.GetResourcePolicy";
}

/// AWS Parameter Store API paths
///
/// AWS Systems Manager Parameter Store uses RESTful paths.
pub mod parameter_store {
    /// Get parameter
    /// GET /systems/manager/getParameter
    pub const GET_PARAMETER: &str = "/systems/manager/getParameter";

    /// Put parameter
    /// PUT /systems/manager/putParameter
    pub const PUT_PARAMETER: &str = "/systems/manager/putParameter";

    /// Delete parameter
    /// DELETE /systems/manager/deleteParameter
    pub const DELETE_PARAMETER: &str = "/systems/manager/deleteParameter";

    /// Get parameters by path
    /// GET /systems/manager/getParametersByPath
    pub const GET_PARAMETERS_BY_PATH: &str = "/systems/manager/getParametersByPath";
}

/// Route constants for Axum routes
///
/// These constants are the single source of truth for Axum route patterns.
/// They are validated against PathBuilder output in tests to ensure consistency.
pub mod routes {
    /// AWS Secrets Manager route patterns
    ///
    /// AWS uses a single POST endpoint "/" with x-amz-target header.
    /// The route constant is the path, and x-amz-target values are in secrets_manager module.
    pub mod secrets_manager {
        /// POST / - All AWS Secrets Manager operations use this route
        /// Operation is specified via x-amz-target header
        pub const BASE: &str = "/";
    }

    /// AWS Parameter Store route patterns
    pub mod parameter_store {
        /// GET /systems/manager/getParameter
        pub const GET_PARAMETER: &str = "/systems/manager/getParameter";

        /// PUT /systems/manager/putParameter
        pub const PUT_PARAMETER: &str = "/systems/manager/putParameter";

        /// DELETE /systems/manager/deleteParameter
        pub const DELETE_PARAMETER: &str = "/systems/manager/deleteParameter";

        /// GET /systems/manager/getParametersByPath
        pub const GET_PARAMETERS_BY_PATH: &str = "/systems/manager/getParametersByPath";
    }
}
