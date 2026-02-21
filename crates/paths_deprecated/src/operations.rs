//! Operation type definitions for PathBuilder
//!
//! This module defines provider-specific operation enums that are wrapped
//! in a main Operation enum for type-safe path construction.

/// Main operation enum that wraps provider-specific operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    Gcp(GcpOperation),
    Aws(AwsOperation),
    Azure(AzureOperation),
}

/// GCP Secret Manager and Parameter Manager operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GcpOperation {
    // Secret Manager operations
    CreateSecret,
    GetSecret,
    UpdateSecret,
    DeleteSecret,
    ListSecrets,

    // Version operations
    AddVersion,
    GetVersion,
    ListVersions,
    EnableVersion,
    DisableVersion,
    EnableSecret,
    DisableSecret,
    AccessVersion, // Get secret value via :access

    // Parameter Manager operations
    CreateParameter,
    GetParameter,
    UpdateParameter,
    DeleteParameter,
    ListParameters,

    // Parameter version operations
    CreateParameterVersion,
    GetParameterVersion,
    ListParameterVersions,
    UpdateParameterVersion,
    DeleteParameterVersion,
    RenderParameterVersion, // Get parameter value via :render

    // Location operations
    GetLocation,
    ListLocations,
}

/// AWS Secrets Manager and Parameter Store operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AwsOperation {
    // Secrets Manager operations
    CreateSecret,
    GetSecretValue,
    DescribeSecret,
    PutSecretValue,
    UpdateSecret,
    DeleteSecret,
    RestoreSecret,
    ListSecrets,

    // Version operations
    ListSecretVersions,
    UpdateSecretVersionStage,

    // Resource operations
    TagResource,
    UntagResource,
    GetResourcePolicy,

    // Parameter Store operations
    GetParameter,
    PutParameter,
    DeleteParameter,
    GetParametersByPath,
}

/// Azure Key Vault and App Configuration operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AzureOperation {
    // Key Vault Secret operations
    SetSecret,
    GetSecret,
    GetSecretVersion,
    ListSecrets,
    ListSecretVersions,
    UpdateSecret, // PATCH for enabled/disabled
    DeleteSecret,

    // Key Vault Secret version operations
    BackupSecret,
    RestoreSecret,

    // Key Vault deleted secret operations
    GetDeletedSecret,
    ListDeletedSecrets,
    RecoverDeletedSecret,
    PurgeDeletedSecret,

    // App Configuration operations
    GetKeyValue,
    SetKeyValue,
    DeleteKeyValue,
    ListKeyValues,
}
