//! PathBuilder implementation
//!
//! Provides a type-safe builder pattern for constructing API paths
//! with different output formats for different consumers.

use crate::errors::PathBuilderError;
use crate::formats::PathFormat;
use crate::gcp;
use crate::operations::{AwsOperation, AzureOperation, GcpOperation, Operation};
use crate::provider::Provider;

/// Builder for constructing API paths with type safety
///
/// # Example
///
/// ```rust
/// use paths::prelude::*;
///
/// let path = PathBuilder::new()
///     .gcp_operation(GcpOperation::CreateSecret)
///     .project("my-project")
///     .build_http_path();
/// // Returns: "projects/my-project/secrets"
/// ```
#[derive(Debug, Clone)]
pub struct PathBuilder {
    provider: Option<Provider>,
    operation: Option<Operation>,

    // GCP parameters
    project: Option<String>,
    location: Option<String>,
    secret: Option<String>,
    parameter: Option<String>,
    version: Option<String>,

    // AWS parameters
    region: Option<String>,

    // Azure parameters
    vault_name: Option<String>,

    // Common
    use_trailing_slash: bool, // For Azure
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PathBuilder {
    /// Create a new PathBuilder
    pub fn new() -> Self {
        Self {
            provider: None,
            operation: None,
            project: None,
            location: None,
            secret: None,
            parameter: None,
            version: None,
            region: None,
            vault_name: None,
            use_trailing_slash: false,
        }
    }

    // Provider selection
    pub fn provider(mut self, provider: Provider) -> Self {
        self.provider = Some(provider);
        self
    }

    // Operation selection
    pub fn operation(mut self, operation: Operation) -> Self {
        self.operation = Some(operation);
        // Auto-set provider from operation
        match operation {
            Operation::Gcp(_) => self.provider = Some(Provider::Gcp),
            Operation::Aws(_) => self.provider = Some(Provider::Aws),
            Operation::Azure(_) => self.provider = Some(Provider::Azure),
        }
        self
    }

    // Convenience methods for provider-specific operations
    pub fn gcp_operation(mut self, operation: GcpOperation) -> Self {
        self.provider = Some(Provider::Gcp);
        self.operation = Some(Operation::Gcp(operation));
        self
    }

    pub fn aws_operation(mut self, operation: AwsOperation) -> Self {
        self.provider = Some(Provider::Aws);
        self.operation = Some(Operation::Aws(operation));
        self
    }

    pub fn azure_operation(mut self, operation: AzureOperation) -> Self {
        self.provider = Some(Provider::Azure);
        self.operation = Some(Operation::Azure(operation));
        self
    }

    // GCP parameters
    pub fn project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    pub fn parameter(mut self, parameter: impl Into<String>) -> Self {
        self.parameter = Some(parameter.into());
        self
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    // AWS parameters
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    // Azure parameters
    pub fn vault_name(mut self, vault_name: impl Into<String>) -> Self {
        self.vault_name = Some(vault_name.into());
        self
    }

    pub fn with_trailing_slash(mut self, enabled: bool) -> Self {
        self.use_trailing_slash = enabled;
        self
    }

    // Build methods
    pub fn build_route(&self) -> Result<String, PathBuilderError> {
        self.build(PathFormat::Route)
    }

    pub fn build_http_path(&self) -> Result<String, PathBuilderError> {
        self.build(PathFormat::HttpPath)
    }

    pub fn build_response_name(&self) -> Result<String, PathBuilderError> {
        self.build(PathFormat::ResponseName)
    }

    pub fn build_pact_path(&self) -> Result<String, PathBuilderError> {
        self.build(PathFormat::PactPath)
    }

    // AWS-specific
    pub fn build_aws_header(&self) -> Result<Option<String>, PathBuilderError> {
        self.validate_provider_operation()?;

        match self.operation {
            Some(Operation::Aws(op)) => {
                use crate::aws::secrets_manager;
                let header = match op {
                    AwsOperation::CreateSecret => Some(secrets_manager::CREATE_SECRET),
                    AwsOperation::GetSecretValue => Some(secrets_manager::GET_SECRET_VALUE),
                    AwsOperation::DescribeSecret => Some(secrets_manager::DESCRIBE_SECRET),
                    AwsOperation::PutSecretValue => Some(secrets_manager::PUT_SECRET_VALUE),
                    AwsOperation::UpdateSecret => Some(secrets_manager::UPDATE_SECRET),
                    AwsOperation::DeleteSecret => Some(secrets_manager::DELETE_SECRET),
                    AwsOperation::RestoreSecret => Some(secrets_manager::RESTORE_SECRET),
                    AwsOperation::ListSecrets => Some(secrets_manager::LIST_SECRETS),
                    AwsOperation::ListSecretVersions => Some(secrets_manager::LIST_SECRET_VERSIONS),
                    AwsOperation::UpdateSecretVersionStage => {
                        Some(secrets_manager::UPDATE_SECRET_VERSION_STAGE)
                    }
                    AwsOperation::TagResource => Some(secrets_manager::TAG_RESOURCE),
                    AwsOperation::UntagResource => Some(secrets_manager::UNTAG_RESOURCE),
                    AwsOperation::GetResourcePolicy => Some(secrets_manager::GET_RESOURCE_POLICY),
                    _ => None,
                };
                Ok(header.map(|s| s.to_string()))
            }
            _ => Ok(None),
        }
    }

    // Generic build with format
    pub fn build(&self, format: PathFormat) -> Result<String, PathBuilderError> {
        self.validate_provider_operation()?;

        match self.operation {
            Some(Operation::Gcp(op)) => self.build_gcp_path(op, format),
            Some(Operation::Aws(op)) => self.build_aws_path(op, format),
            Some(Operation::Azure(op)) => self.build_azure_path(op, format),
            None => Err(PathBuilderError::MissingRequiredParameter(
                "operation".to_string(),
            )),
        }
    }

    // Validation: Ensure provider matches operation
    fn validate_provider_operation(&self) -> Result<(), PathBuilderError> {
        match (&self.provider, &self.operation) {
            (Some(Provider::Gcp), Some(Operation::Gcp(_))) => Ok(()),
            (Some(Provider::Aws), Some(Operation::Aws(_))) => Ok(()),
            (Some(Provider::Azure), Some(Operation::Azure(_))) => Ok(()),
            (Some(_), Some(_)) => Err(PathBuilderError::ProviderOperationMismatch),
            (None, Some(_)) => Ok(()), // Provider will be auto-set from operation
            (Some(_), None) => Err(PathBuilderError::MissingRequiredParameter(
                "operation".to_string(),
            )),
            (None, None) => Err(PathBuilderError::MissingRequiredParameter(
                "operation".to_string(),
            )),
        }
    }

    // GCP path building
    fn build_gcp_path(
        &self,
        op: GcpOperation,
        format: PathFormat,
    ) -> Result<String, PathBuilderError> {
        let project = self
            .project
            .as_deref()
            .ok_or_else(|| PathBuilderError::MissingRequiredParameter("project".to_string()))?;

        match op {
            GcpOperation::CreateSecret | GcpOperation::ListSecrets => {
                let path = gcp::secret_manager::create_secret(project);
                Ok(self.format_path(path, format))
            }
            GcpOperation::GetSecret | GcpOperation::UpdateSecret | GcpOperation::DeleteSecret => {
                let secret = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = gcp::secret_manager::get_secret_metadata(project, secret);
                Ok(self.format_path(path, format))
            }
            GcpOperation::AddVersion => {
                let secret = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = gcp::secret_manager::add_version(project, secret);
                Ok(self.format_path(path, format))
            }
            GcpOperation::GetVersion
            | GcpOperation::EnableVersion
            | GcpOperation::DisableVersion => {
                let secret = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let version = self.version.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("version".to_string())
                })?;
                let path = gcp::secret_manager::get_version(project, secret, version);
                Ok(self.format_path(path, format))
            }
            GcpOperation::ListVersions => {
                let secret = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = gcp::secret_manager::list_versions(project, secret);
                Ok(self.format_path(path, format))
            }
            GcpOperation::EnableSecret | GcpOperation::DisableSecret => {
                let secret = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = match op {
                    GcpOperation::EnableSecret => {
                        gcp::secret_manager::enable_secret(project, secret)
                    }
                    GcpOperation::DisableSecret => {
                        gcp::secret_manager::disable_secret(project, secret)
                    }
                    _ => unreachable!(),
                };
                Ok(self.format_path(path, format))
            }
            GcpOperation::AccessVersion => {
                let secret = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let version = self.version.as_deref().unwrap_or("latest");
                let path = gcp::secret_manager::access_version(project, secret, version);
                Ok(self.format_path(path, format))
            }
            // Parameter Manager operations
            GcpOperation::CreateParameter | GcpOperation::ListParameters => {
                let location = self.location.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("location".to_string())
                })?;
                let path = gcp::parameter_manager::create_parameter(project, location);
                Ok(self.format_path(path, format))
            }
            GcpOperation::GetParameter
            | GcpOperation::UpdateParameter
            | GcpOperation::DeleteParameter => {
                let location = self.location.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("location".to_string())
                })?;
                let parameter = self.parameter.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("parameter".to_string())
                })?;
                let path = gcp::parameter_manager::get_parameter(project, location, parameter);
                Ok(self.format_path(path, format))
            }
            GcpOperation::CreateParameterVersion | GcpOperation::ListParameterVersions => {
                let location = self.location.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("location".to_string())
                })?;
                let parameter = self.parameter.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("parameter".to_string())
                })?;
                let path = gcp::parameter_manager::create_version(project, location, parameter);
                Ok(self.format_path(path, format))
            }
            GcpOperation::GetParameterVersion
            | GcpOperation::UpdateParameterVersion
            | GcpOperation::DeleteParameterVersion => {
                let location = self.location.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("location".to_string())
                })?;
                let parameter = self.parameter.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("parameter".to_string())
                })?;
                let version = self.version.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("version".to_string())
                })?;
                let path =
                    gcp::parameter_manager::get_version(project, location, parameter, version);
                Ok(self.format_path(path, format))
            }
            GcpOperation::RenderParameterVersion => {
                let location = self.location.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("location".to_string())
                })?;
                let parameter = self.parameter.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("parameter".to_string())
                })?;
                let version = self.version.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("version".to_string())
                })?;
                let path =
                    gcp::parameter_manager::render_version(project, location, parameter, version);
                Ok(self.format_path(path, format))
            }
            // Location operations
            GcpOperation::GetLocation => {
                let location = self.location.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("location".to_string())
                })?;
                let path = gcp::parameter_manager::get_location(project, location);
                Ok(self.format_path(path, format))
            }
            GcpOperation::ListLocations => {
                let path = gcp::parameter_manager::list_locations(project);
                Ok(self.format_path(path, format))
            }
        }
    }

    // AWS path building
    fn build_aws_path(
        &self,
        _op: AwsOperation,
        format: PathFormat,
    ) -> Result<String, PathBuilderError> {
        // AWS uses a single POST endpoint "/" with x-amz-target header
        match format {
            PathFormat::Route | PathFormat::HttpPath | PathFormat::PactPath => Ok("/".to_string()),
            PathFormat::ResponseName => Err(PathBuilderError::InvalidFormatForOperation),
        }
    }

    // Azure path building
    fn build_azure_path(
        &self,
        op: AzureOperation,
        format: PathFormat,
    ) -> Result<String, PathBuilderError> {
        use crate::azure::key_vault as kv;

        match op {
            AzureOperation::ListSecrets => {
                let path = kv::LIST_SECRETS.to_string();
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::GetSecret => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = if self.use_trailing_slash {
                    kv::get_secret(name)
                } else {
                    kv::set_secret(name) // Same path, different method
                };
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::GetSecretVersion => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let version = self.version.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("version".to_string())
                })?;
                let path = kv::get_secret_version(name, version);
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::ListSecretVersions => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = kv::list_secret_versions(name);
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::SetSecret => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = kv::set_secret(name);
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::DeleteSecret => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = kv::delete_secret(name);
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::UpdateSecret => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = kv::update_secret(name);
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::BackupSecret => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = kv::backup_secret(name);
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::RestoreSecret => {
                let path = kv::RESTORE_SECRET.to_string();
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::GetDeletedSecret => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = kv::get_deleted_secret(name);
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::ListDeletedSecrets => {
                let path = kv::LIST_DELETED_SECRETS.to_string();
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::RecoverDeletedSecret => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = kv::recover_deleted_secret(name);
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::PurgeDeletedSecret => {
                let name = self.secret.as_deref().ok_or_else(|| {
                    PathBuilderError::MissingRequiredParameter("secret".to_string())
                })?;
                let path = kv::purge_deleted_secret(name);
                Ok(self.format_azure_path(path, format))
            }
            // App Configuration operations
            AzureOperation::GetKeyValue
            | AzureOperation::SetKeyValue
            | AzureOperation::DeleteKeyValue => {
                use crate::azure::app_configuration as ac;
                let key = self
                    .secret
                    .as_deref()
                    .ok_or_else(|| PathBuilderError::MissingRequiredParameter("key".to_string()))?;
                let path = match op {
                    AzureOperation::GetKeyValue => ac::get_key_value(key),
                    AzureOperation::SetKeyValue => ac::set_key_value(key),
                    AzureOperation::DeleteKeyValue => ac::delete_key_value(key),
                    _ => unreachable!(),
                };
                Ok(self.format_azure_path(path, format))
            }
            AzureOperation::ListKeyValues => {
                use crate::azure::app_configuration as ac;
                let path = ac::LIST_KEY_VALUES.to_string();
                Ok(self.format_azure_path(path, format))
            }
        }
    }

    // Format Azure path based on output format
    fn format_azure_path(&self, path: String, format: PathFormat) -> String {
        match format {
            PathFormat::Route => {
                // Replace actual values with placeholders for route patterns
                let mut route = path;

                // Replace secret/name if present
                if let Some(secret) = &self.secret {
                    route = route.replace(secret, "{name}");
                }

                // Replace version if present
                if let Some(version) = &self.version {
                    route = route.replace(version, "{version}");
                }

                route
            }
            PathFormat::HttpPath => {
                // Remove /v1/ prefix if present - make_request will add it
                path.strip_prefix("/v1/").unwrap_or(&path).to_string()
            }
            PathFormat::ResponseName => {
                // Azure doesn't use response names like GCP, return as-is
                path
            }
            PathFormat::PactPath => path,
        }
    }

    // Format path based on output format
    fn format_path(&self, path: String, format: PathFormat) -> String {
        match format {
            PathFormat::Route => {
                // Replace actual values with placeholders for route patterns
                // We need to replace in reverse order (longer strings first) to avoid partial replacements
                let mut route = path;

                // Replace version first (longest, most specific)
                if let Some(version) = &self.version {
                    route = route.replace(version, "{version}");
                }

                // Replace parameter
                if let Some(parameter) = &self.parameter {
                    route = route.replace(parameter, "{parameter}");
                }

                // Replace location
                if let Some(location) = &self.location {
                    route = route.replace(location, "{location}");
                }

                // Replace secret
                if let Some(secret) = &self.secret {
                    route = route.replace(secret, "{secret}");
                }

                // Replace project last (shortest, most common)
                if let Some(project) = &self.project {
                    route = route.replace(project, "{project}");
                }

                route
            }
            PathFormat::HttpPath => {
                // Remove /v1/ prefix if present - make_request will add it
                path.strip_prefix("/v1/").unwrap_or(&path).to_string()
            }
            PathFormat::ResponseName => {
                // Remove /v1/ prefix if present
                path.strip_prefix("/v1/").unwrap_or(&path).to_string()
            }
            PathFormat::PactPath => path,
        }
    }
}
