//! Secret size limit validation
//!
//! Each cloud provider has different size limits for secret values:
//! - GCP: 64KB (65,536 bytes)
//! - AWS: 64KB (65,536 bytes)
//! - Azure: 25KB (25,600 bytes)
//!
//! These limits apply to the decoded secret value (not the base64-encoded payload).

use base64::{Engine as _, engine::general_purpose};

/// GCP Secret Manager size limit: 64KB
pub const GCP_SECRET_SIZE_LIMIT: usize = 64 * 1024; // 65,536 bytes

/// AWS Secrets Manager size limit: 64KB
pub const AWS_SECRET_SIZE_LIMIT: usize = 64 * 1024; // 65,536 bytes

/// Azure Key Vault size limit: 25KB
pub const AZURE_SECRET_SIZE_LIMIT: usize = 25 * 1024; // 25,600 bytes

/// Validate secret size for GCP
///
/// Returns Ok(()) if the secret is within the limit, or an error message if it exceeds the limit.
/// The input is expected to be base64-encoded data.
pub fn validate_gcp_secret_size(base64_data: &str) -> Result<(), String> {
    // Decode base64 to get actual size
    let decoded = match general_purpose::STANDARD.decode(base64_data) {
        Ok(data) => data,
        Err(e) => return Err(format!("Invalid base64 data: {e}")),
    };

    if decoded.len() > GCP_SECRET_SIZE_LIMIT {
        return Err(format!(
            "Secret size {} bytes exceeds GCP limit of {GCP_SECRET_SIZE_LIMIT} bytes (64KB)",
            decoded.len()
        ));
    }

    Ok(())
}

/// Validate secret size for AWS
///
/// Returns Ok(()) if the secret is within the limit, or an error message if it exceeds the limit.
/// The input can be either plain text or base64-encoded data.
pub fn validate_aws_secret_size(secret_value: &str) -> Result<(), String> {
    // AWS accepts both plain text and base64-encoded values
    // We'll check the plain text size (if it's base64, the decoded size would be smaller)
    // For simplicity, we check the string length in bytes
    let size = secret_value.as_bytes().len();

    if size > AWS_SECRET_SIZE_LIMIT {
        return Err(format!(
            "Secret size {size} bytes exceeds AWS limit of {AWS_SECRET_SIZE_LIMIT} bytes (64KB)"
        ));
    }

    Ok(())
}

/// Validate secret size for Azure
///
/// Returns Ok(()) if the secret is within the limit, or an error message if it exceeds the limit.
/// The input is expected to be plain text (Azure doesn't use base64 encoding in the API).
pub fn validate_azure_secret_size(secret_value: &str) -> Result<(), String> {
    let size = secret_value.as_bytes().len();

    if size > AZURE_SECRET_SIZE_LIMIT {
        return Err(format!(
            "Secret size {size} bytes exceeds Azure limit of {AZURE_SECRET_SIZE_LIMIT} bytes (25KB)"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcp_secret_size_validation() {
        // Valid size (1KB)
        let small_data = general_purpose::STANDARD.encode(&vec![0u8; 1024]);
        assert!(validate_gcp_secret_size(&small_data).is_ok());

        // Exactly at limit (64KB)
        let limit_data = general_purpose::STANDARD.encode(&vec![0u8; GCP_SECRET_SIZE_LIMIT]);
        assert!(validate_gcp_secret_size(&limit_data).is_ok());

        // Exceeds limit (65KB)
        let large_data = general_purpose::STANDARD.encode(&vec![0u8; GCP_SECRET_SIZE_LIMIT + 1024]);
        assert!(validate_gcp_secret_size(&large_data).is_err());
    }

    #[test]
    fn test_aws_secret_size_validation() {
        // Valid size
        let small_secret = "a".repeat(1024);
        assert!(validate_aws_secret_size(&small_secret).is_ok());

        // Exactly at limit
        let limit_secret = "a".repeat(AWS_SECRET_SIZE_LIMIT);
        assert!(validate_aws_secret_size(&limit_secret).is_ok());

        // Exceeds limit
        let large_secret = "a".repeat(AWS_SECRET_SIZE_LIMIT + 1024);
        assert!(validate_aws_secret_size(&large_secret).is_err());
    }

    #[test]
    fn test_azure_secret_size_validation() {
        // Valid size
        let small_secret = "a".repeat(1024);
        assert!(validate_azure_secret_size(&small_secret).is_ok());

        // Exactly at limit
        let limit_secret = "a".repeat(AZURE_SECRET_SIZE_LIMIT);
        assert!(validate_azure_secret_size(&limit_secret).is_ok());

        // Exceeds limit
        let large_secret = "a".repeat(AZURE_SECRET_SIZE_LIMIT + 1024);
        assert!(validate_azure_secret_size(&large_secret).is_err());
    }
}
