//! # Logging Configuration
//!
//! Log level configuration for the Secret Manager Controller.
//!
//! Allows fine-grained control over logging verbosity for different operations.

use serde::{Deserialize, Serialize};

/// Log level configuration
///
/// Controls the verbosity of logging for different operations.
/// Log levels follow standard hierarchy: DEBUG includes INFO and WARN, WARN includes INFO.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    /// ERROR level - only log errors
    Error,
    /// WARN level - log warnings and errors (includes ERROR)
    Warn,
    /// INFO level - log informational messages, warnings, and errors (includes WARN, ERROR)
    Info,
    /// DEBUG level - log all messages including detailed debugging information (includes INFO, WARN, ERROR)
    Debug,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Info
    }
}

impl LogLevel {
    /// Check if a log level should be emitted
    ///
    /// Returns true if the requested level should be logged based on the configured level.
    /// Level hierarchy: DEBUG > INFO > WARN > ERROR
    pub fn should_log(&self, requested_level: &LogLevel) -> bool {
        match (self, requested_level) {
            (LogLevel::Debug, _) => true,
            (LogLevel::Info, LogLevel::Info | LogLevel::Warn | LogLevel::Error) => true,
            (LogLevel::Warn, LogLevel::Warn | LogLevel::Error) => true,
            (LogLevel::Error, LogLevel::Error) => true,
            _ => false,
        }
    }

    /// Get the tracing level equivalent
    pub fn to_tracing_level(&self) -> tracing::Level {
        match self {
            LogLevel::Error => tracing::Level::ERROR,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Debug => tracing::Level::DEBUG,
        }
    }
}

/// Logging configuration for different operations
///
/// Allows setting different log levels for different types of operations.
/// This provides fine-grained control over log verbosity.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoggingConfig {
    /// Log level for secret operations (create, update, delete, enable, disable)
    /// Default: INFO
    #[serde(default = "default_info_level")]
    pub secrets: LogLevel,
    /// Log level for property/config operations (create, update, delete)
    /// Default: INFO
    #[serde(default = "default_info_level")]
    pub properties: LogLevel,
    /// Log level for reconciliation operations (start, complete, errors)
    /// Default: INFO
    #[serde(default = "default_info_level")]
    pub reconciliation: LogLevel,
    /// Log level for diff discovery operations (comparing Git vs cloud provider)
    /// Default: WARN (only log when differences are found)
    #[serde(default = "default_warn_level")]
    pub diff_discovery: LogLevel,
    /// Log level for SOPS decryption operations
    /// Default: DEBUG (detailed decryption process)
    #[serde(default = "default_debug_level")]
    pub sops: LogLevel,
    /// Log level for Git/artifact operations (clone, pull, resolve)
    /// Default: INFO
    #[serde(default = "default_info_level")]
    pub git: LogLevel,
    /// Log level for provider operations (authentication, API calls)
    /// Default: DEBUG (detailed API interactions)
    #[serde(default = "default_debug_level")]
    pub provider: LogLevel,
    /// Log level for Kustomize operations
    /// Default: INFO
    #[serde(default = "default_info_level")]
    pub kustomize: LogLevel,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        LoggingConfig {
            secrets: LogLevel::Info,
            properties: LogLevel::Info,
            reconciliation: LogLevel::Info,
            diff_discovery: LogLevel::Warn,
            sops: LogLevel::Debug,
            git: LogLevel::Info,
            provider: LogLevel::Debug,
            kustomize: LogLevel::Info,
        }
    }
}

/// Default log level functions for serde defaults
fn default_info_level() -> LogLevel {
    LogLevel::Info
}

fn default_warn_level() -> LogLevel {
    LogLevel::Warn
}

fn default_debug_level() -> LogLevel {
    LogLevel::Debug
}
