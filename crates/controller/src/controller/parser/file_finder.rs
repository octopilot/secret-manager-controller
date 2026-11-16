//! # File Finder
//!
//! Discovers application configuration files in repository structures.

use crate::controller::parser::types::ApplicationFiles;
use anyhow::Result;
use std::path::Path;
use tracing::warn;
use walkdir::WalkDir;

/// Normalize base path - handle "." and "" as empty/root
#[cfg(test)]
pub fn normalize_base_path(base_path: Option<&str>) -> Option<&str> {
    normalize_base_path_impl(base_path)
}

fn normalize_base_path_impl(base_path: Option<&str>) -> Option<&str> {
    match base_path {
        Some("." | "") | None => None,
        Some(path) => Some(path),
    }
}

/// Find application files for a specific environment/profile
/// Supports both monolith and single service structures:
/// - Monolith: {basePath}/{service}/deployment-configuration/profiles/{env}/
/// - Single service: deployment-configuration/profiles/{env}/
/// - Backward compatible: deployment-configuration/{env}/ (without profiles)
///
/// Only processes the specified environment name - does not scan all environments
///
/// If `base_path` is None, searches from repository root
#[allow(
    clippy::unused_async,
    clippy::missing_errors_doc,
    reason = "May be called from async contexts in the future, error docs in comments"
)]
pub async fn find_application_files(
    artifact_path: &Path,
    base_path: Option<&str>,
    environment: &str,
    default_service_name: Option<&str>,
) -> Result<Vec<ApplicationFiles>> {
    // Normalize base path - handle "." and "" as root
    let normalized_base = normalize_base_path_impl(base_path);
    let search_path = match normalized_base {
        None => artifact_path.to_path_buf(),
        Some(path) => artifact_path.join(path),
    };

    if !search_path.exists() {
        warn!("Base path does not exist: {}", search_path.display());
        return Ok(vec![]);
    }

    let mut application_files = Vec::new();

    // Walk through directory structure
    // Expected structures:
    // - Monolith: microservices/{service}/deployment-configuration/profiles/{env}/
    // - Single service: deployment-configuration/profiles/{env}/
    // - Legacy: deployment-configuration/{env}/ (backward compatibility)
    for entry in WalkDir::new(&search_path)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();

        // Check if this is a deployment-configuration directory
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "deployment-configuration")
        {
            // Extract service name (parent of deployment-configuration)
            let service_name = if let Some(parent) = path.parent() {
                // Check if parent is the base_path (single service case)
                if parent == search_path {
                    // Single service: use default_service_name or fallback
                    default_service_name
                        .map(ToString::to_string)
                        .or_else(|| {
                            // Try to extract from artifact path or use default
                            artifact_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(ToString::to_string)
                        })
                        .unwrap_or_else(|| "default-service".to_string())
                } else {
                    // Monolith: extract service name from parent directory
                    parent
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map_or_else(|| "unknown".to_string(), ToString::to_string)
                }
            } else {
                default_service_name.map_or("unknown".to_string(), ToString::to_string)
            };

            // Look for profiles directory first (Skaffold-compliant structure)
            let profiles_path = path.join("profiles");
            if profiles_path.exists() && profiles_path.is_dir() {
                // New structure: deployment-configuration/profiles/{env}/
                // Only process the specified environment
                let env_path = profiles_path.join(environment);
                if env_path.exists() && env_path.is_dir() {
                    let app_files = find_files_in_directory(&service_name, &env_path)?;
                    if app_files.has_any_files() {
                        application_files.push(app_files);
                    } else {
                        warn!(
                            "No application files found in environment '{}' at {}",
                            environment,
                            env_path.display()
                        );
                    }
                } else {
                    warn!(
                        "Environment '{}' not found in profiles directory: {}",
                        environment,
                        profiles_path.display()
                    );
                }
            } else {
                // Backward compatibility: deployment-configuration/{env}/ (without profiles)
                // Only process the specified environment
                let env_path = path.join(environment);
                if env_path.exists() && env_path.is_dir() {
                    let app_files = find_files_in_directory(&service_name, &env_path)?;
                    if app_files.has_any_files() {
                        application_files.push(app_files);
                    } else {
                        warn!(
                            "No application files found in environment '{}' at {}",
                            environment,
                            env_path.display()
                        );
                    }
                } else {
                    warn!(
                        "Environment '{}' not found in deployment-configuration directory: {}",
                        environment,
                        path.display()
                    );
                }
            }
        }
    }

    Ok(application_files)
}

/// Application files for an explicit list of secret files (`spec.secrets.files`).
///
/// Each path is relative to the source root and must name a file inside it. The file
/// name decides how it is read: `.env` is dotenv, `.yaml`, `.yml` and `.json` are YAML.
/// One `ApplicationFiles` is returned per file, so files of the same kind do not
/// displace each other.
#[allow(
    clippy::missing_errors_doc,
    reason = "Error documentation is provided in doc comments"
)]
pub fn explicit_application_files(
    artifact_path: &Path,
    files: &[String],
    service_name: &str,
) -> Result<Vec<ApplicationFiles>> {
    use std::path::Component;

    let mut application_files = Vec::with_capacity(files.len());
    for file in files {
        let relative = Path::new(file);
        let inside_source = !file.is_empty()
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        if !inside_source {
            anyhow::bail!("Secret file '{file}' must be a path relative to the source root");
        }
        let path = artifact_path.join(relative);
        if !path.is_file() {
            anyhow::bail!("Secret file '{file}' not found in the source");
        }
        let mut app_files = ApplicationFiles {
            service_name: service_name.to_string(),
            base_path: path.parent().unwrap_or(artifact_path).to_path_buf(),
            secrets_env: None,
            secrets_yaml: None,
            properties: None,
        };
        match path.extension().and_then(|e| e.to_str()) {
            Some("env") => app_files.secrets_env = Some(path),
            Some("yaml" | "yml" | "json") => app_files.secrets_yaml = Some(path),
            _ => anyhow::bail!(
                "Secret file '{file}' has an unsupported type: expected .env, .yaml, .yml or .json"
            ),
        }
        application_files.push(app_files);
    }
    Ok(application_files)
}

fn find_files_in_directory(service_name: &str, dir: &Path) -> Result<ApplicationFiles> {
    let mut app_files = ApplicationFiles {
        service_name: service_name.to_string(),
        base_path: dir.to_path_buf(),
        secrets_env: None,
        secrets_yaml: None,
        properties: None,
    };

    // Look for application files
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            match file_name {
                "application.secrets.env" => {
                    app_files.secrets_env = Some(path);
                }
                "application.secrets.yaml" => {
                    app_files.secrets_yaml = Some(path);
                }
                "application.properties" => {
                    app_files.properties = Some(path);
                }
                _ => {}
            }
        }
    }

    Ok(app_files)
}
