//! # Types
//!
//! Data structures for application file parsing.

use std::path::PathBuf;

/// Application files found in a directory
#[derive(Debug, Clone)]
pub struct ApplicationFiles {
    pub service_name: String,
    pub base_path: PathBuf,
    pub secrets_env: Option<PathBuf>,
    pub secrets_yaml: Option<PathBuf>,
    pub properties: Option<PathBuf>,
}

impl ApplicationFiles {
    /// Check if any application files are present
    #[must_use]
    pub fn has_any_files(&self) -> bool {
        self.secrets_env.is_some() || self.secrets_yaml.is_some() || self.properties.is_some()
    }
}
