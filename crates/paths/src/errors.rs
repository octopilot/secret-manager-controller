//! Error types for PathBuilder

use std::fmt;

/// Errors that can occur during path construction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathBuilderError {
    /// Provider and operation don't match
    ProviderOperationMismatch,

    /// Required parameter is missing
    MissingRequiredParameter(String),

    /// Invalid operation for the selected provider
    InvalidOperationForProvider,

    /// Invalid format for the operation
    InvalidFormatForOperation,
}

impl fmt::Display for PathBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathBuilderError::ProviderOperationMismatch => {
                write!(f, "Provider and operation do not match")
            }
            PathBuilderError::MissingRequiredParameter(param) => {
                write!(f, "Missing required parameter: {param}")
            }
            PathBuilderError::InvalidOperationForProvider => {
                write!(f, "Invalid operation for the selected provider")
            }
            PathBuilderError::InvalidFormatForOperation => {
                write!(f, "Invalid format for the operation")
            }
        }
    }
}

impl std::error::Error for PathBuilderError {}
