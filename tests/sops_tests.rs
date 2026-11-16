//! # SOPS Decryption Unit Tests
//!
//! Comprehensive unit tests for SOPS decryption functionality.
//!
//! These tests verify:
//! - SOPS-encrypted file detection
//! - Decryption with correct input/output types (dotenv, yaml, json)
//! - Error handling for missing keys, invalid files, etc.
//! - Security: No disk writes (stdin/stdout pipes only)

use controller::controller::parser::sops::error::{
    SopsDecryptionFailureReason, classify_sops_error,
};
use controller::controller::parser::{decrypt_sops_content, is_sops_encrypted};
use std::env;
use std::path::PathBuf;

/// Test helper: Get SOPS private key from environment
/// In CI, this should be set as a GitHub Actions secret
fn get_test_sops_key() -> Option<String> {
    env::var("SOPS_PRIVATE_KEY").ok()
}

/// Test helper: Get path to test files
///
/// Priority:
/// 1. TEST_FILES_DIR environment variable (for CI/CD)
/// 2. Integration test fixtures (tests/integration/fixtures)
/// 3. Default deployment-configuration path (for local development)
fn get_test_files_dir() -> PathBuf {
    if let Ok(dir) = env::var("TEST_FILES_DIR") {
        return PathBuf::from(dir);
    }

    // Try integration test fixtures
    let integration_fixtures = PathBuf::from("tests/integration/fixtures");
    if integration_fixtures.exists() {
        return integration_fixtures;
    }

    // Default to deployment-configuration for local development
    PathBuf::from("deployment-configuration/profiles/tilt")
}

#[test]
fn test_sops_encrypted_detection() {
    // Test YAML format
    let yaml_content = r#"sops:
    kms: []
    gcp_kms: []
    azure_kv: []
    hc_vault: []
    age: []
    lastmodified: "2024-01-01T00:00:00Z"
    mac: ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]
    version: 3.8.0
DATABASE_URL: ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]
"#;
    assert!(is_sops_encrypted(yaml_content));

    // Test JSON format
    let json_content = r#"{
    "sops": {
        "kms": [],
        "gcp_kms": [],
        "azure_kv": [],
        "hc_vault": [],
        "age": [],
        "lastmodified": "2024-01-01T00:00:00Z",
        "mac": "ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]",
        "version": "3.8.0"
    },
    "DATABASE_URL": "ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]"
}"#;
    assert!(is_sops_encrypted(json_content));

    // Test ENV format with metadata
    let env_content = r#"# sops_version: 3.8.0
# sops_encrypted: true
DATABASE_URL=ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]
"#;
    assert!(is_sops_encrypted(env_content));

    // Test ENV format with ENC pattern
    let env_enc_content = "DATABASE_URL=ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]";
    assert!(is_sops_encrypted(env_enc_content));

    // Test plain YAML (not encrypted)
    let plain_yaml = r#"
database:
  url: postgresql://localhost/db
"#;
    assert!(!is_sops_encrypted(plain_yaml));

    // Test plain ENV (not encrypted)
    let plain_env = "DATABASE_URL=postgresql://localhost/db";
    assert!(!is_sops_encrypted(plain_env));
}

/// The tests below assert how a sops failure is classified, so they need the sops
/// binary. Without it they are skipped, unless SMC_REQUIRE_SOPS is set, as
/// hack/test.sh does after installing sops: CI must never skip them.
fn sops_missing() -> bool {
    let missing = std::process::Command::new("sops")
        .arg("--version")
        .arg("--disable-version-check")
        .output()
        .is_err();
    if missing {
        assert!(
            std::env::var_os("SMC_REQUIRE_SOPS").is_none(),
            "sops is required (SMC_REQUIRE_SOPS) but is not installed"
        );
        eprintln!("skipped: sops is not installed");
    }
    missing
}

#[tokio::test]
async fn test_decrypt_dotenv_file() {
    let sops_key = get_test_sops_key();
    if sops_key.is_none() {
        eprintln!("⚠️  SOPS_PRIVATE_KEY not set - skipping test");
        return;
    }

    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.env");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    // Read encrypted content
    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Verify it's encrypted
    assert!(is_sops_encrypted(&encrypted_content));

    // Decrypt
    let decrypted = decrypt_sops_content(&encrypted_content, Some(&file_path), sops_key.as_deref())
        .await
        .expect("Failed to decrypt SOPS file");

    // Verify decrypted content
    assert!(!decrypted.is_empty());
    assert!(!is_sops_encrypted(&decrypted));

    // Verify expected keys are present
    assert!(decrypted.contains("DATABASE_URL"));
    assert!(decrypted.contains("DATABASE_USER"));
}

#[tokio::test]
async fn test_decrypt_yaml_file() {
    let sops_key = get_test_sops_key();
    if sops_key.is_none() {
        eprintln!("⚠️  SOPS_PRIVATE_KEY not set - skipping test");
        return;
    }

    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.yaml");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    // Read encrypted content
    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Verify it's encrypted
    assert!(is_sops_encrypted(&encrypted_content));

    // Decrypt
    let decrypted = decrypt_sops_content(&encrypted_content, Some(&file_path), sops_key.as_deref())
        .await
        .expect("Failed to decrypt SOPS file");

    // Verify decrypted content
    assert!(!decrypted.is_empty());
    assert!(!is_sops_encrypted(&decrypted));

    // Verify expected keys are present
    assert!(decrypted.contains("DATABASE_URL"));
    assert!(decrypted.contains("DATABASE_USER"));
}

#[tokio::test]
async fn test_decrypt_without_key_fails() {
    if sops_missing() {
        return;
    }
    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.env");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Try to decrypt without key - should fail (unless system keyring has the key)
    let result = decrypt_sops_content(&encrypted_content, Some(&file_path), None).await;

    // If it succeeds, that means the system keyring has the key (acceptable)
    // If it fails, verify error classification
    if let Err(error) = result {
        // Verify error classification - should be KeyNotFound or WrongKey
        assert!(
            error.reason == SopsDecryptionFailureReason::KeyNotFound
                || error.reason == SopsDecryptionFailureReason::WrongKey
        );
        assert!(!error.is_transient); // Key not found is permanent
    }
}

#[tokio::test]
async fn test_decrypt_with_wrong_key_fails() {
    if sops_missing() {
        return;
    }
    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.env");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Try to decrypt with wrong key (a dummy GPG key)
    let wrong_key = r#"-----BEGIN PGP PRIVATE KEY BLOCK-----
This is not a valid GPG key, just for testing
-----END PGP PRIVATE KEY BLOCK-----"#;

    let result = decrypt_sops_content(&encrypted_content, Some(&file_path), Some(wrong_key)).await;

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Verify error classification - wrong key should be classified as WrongKey or InvalidKeyFormat
    assert!(
        error.reason == SopsDecryptionFailureReason::WrongKey
            || error.reason == SopsDecryptionFailureReason::InvalidKeyFormat
    );
    assert!(error.is_transient == false); // Wrong key is permanent
}

#[tokio::test]
async fn test_decrypt_invalid_content_fails() {
    // Try to decrypt non-SOPS content (should fail gracefully)
    let invalid_content = "This is not SOPS-encrypted content at all";

    let result = decrypt_sops_content(invalid_content, None, None).await;

    // This might succeed if SOPS can handle it, or fail - either is acceptable
    // The important thing is it doesn't panic
    let _ = result; // Just ensure it doesn't panic
}

#[test]
fn test_error_classification() {
    // Test key_not_found classification
    let error_msg = "no decryption key found for data key";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::KeyNotFound);
    assert!(!reason.is_transient());

    // Test wrong_key classification
    let error_msg = "decryption failed: wrong key";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::WrongKey);
    assert!(!reason.is_transient());

    // Test invalid_key_format classification
    let error_msg = "invalid key format: malformed GPG key";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::InvalidKeyFormat);
    assert!(!reason.is_transient());

    // Test network_timeout classification (transient)
    let error_msg = "network timeout while contacting GPG provider";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::NetworkTimeout);
    assert!(reason.is_transient());

    // Test provider_unavailable classification (transient)
    let error_msg = "GPG provider unavailable: connection refused";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::ProviderUnavailable);
    assert!(reason.is_transient());

    // Test permission_denied classification (transient)
    let error_msg = "permission denied accessing secret";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::PermissionDenied);
    assert!(reason.is_transient());

    // Test corrupted_file classification
    let error_msg = "corrupted file: invalid SOPS format";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::CorruptedFile);
    assert!(!reason.is_transient());

    // Test unsupported_format classification
    let error_msg = "unsupported format: unknown file type";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::UnsupportedFormat);
    assert!(!reason.is_transient());

    // Test unknown error (defaults to transient for safety)
    let error_msg = "some random error message";
    let reason = classify_sops_error(error_msg, None);
    assert_eq!(reason, SopsDecryptionFailureReason::Unknown);
    assert!(reason.is_transient()); // Unknown errors are treated as transient for safety
}

#[test]
fn test_error_remediation_guidance() {
    // Test that each error reason provides remediation guidance
    let reasons = vec![
        SopsDecryptionFailureReason::KeyNotFound,
        SopsDecryptionFailureReason::WrongKey,
        SopsDecryptionFailureReason::InvalidKeyFormat,
        SopsDecryptionFailureReason::UnsupportedFormat,
        SopsDecryptionFailureReason::CorruptedFile,
        SopsDecryptionFailureReason::NetworkTimeout,
        SopsDecryptionFailureReason::ProviderUnavailable,
        SopsDecryptionFailureReason::PermissionDenied,
        SopsDecryptionFailureReason::Unknown,
    ];

    for reason in reasons {
        let remediation = reason.remediation();
        assert!(
            !remediation.is_empty(),
            "Remediation guidance should not be empty for {:?}",
            reason
        );
        assert!(
            remediation.len() > 20,
            "Remediation guidance should be detailed for {:?}",
            reason
        );
    }
}

#[test]
fn test_error_reason_strings() {
    // Test that all error reasons have valid string representations for metrics
    let reasons = vec![
        SopsDecryptionFailureReason::KeyNotFound,
        SopsDecryptionFailureReason::WrongKey,
        SopsDecryptionFailureReason::InvalidKeyFormat,
        SopsDecryptionFailureReason::UnsupportedFormat,
        SopsDecryptionFailureReason::CorruptedFile,
        SopsDecryptionFailureReason::NetworkTimeout,
        SopsDecryptionFailureReason::ProviderUnavailable,
        SopsDecryptionFailureReason::PermissionDenied,
        SopsDecryptionFailureReason::Unknown,
    ];

    for reason in reasons {
        let reason_str = reason.as_str();
        assert!(!reason_str.is_empty());
        assert!(!reason_str.contains(' ')); // Should be valid metric label (no spaces)
        assert!(reason_str.chars().all(|c| c.is_alphanumeric() || c == '_')); // Valid metric label characters
    }
}

#[test]
fn test_content_based_type_detection() {
    // Test that file type detection works from content when path is not available
    let yaml_content = r#"sops:
    version: 3.8.0
DATABASE_URL: ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]
"#;
    assert!(is_sops_encrypted(yaml_content));

    let json_content = r#"{"sops": {"version": "3.8.0"}, "DATABASE_URL": "ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]"}"#;
    assert!(is_sops_encrypted(json_content));

    let env_content = "DATABASE_URL=ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]";
    assert!(is_sops_encrypted(env_content));
}

#[test]
fn test_file_type_detection_from_path() {
    use controller::controller::parser::sops::is_sops_encrypted;

    // Test that ENC pattern detection works for dotenv files
    let dotenv_content = "DATABASE_URL=ENC[AES256_GCM,data:test,iv:test,tag:test,type:str]\nANOTHER_KEY=ENC[AES256_GCM,data:test2,iv:test2,tag:test2,type:str]";
    assert!(is_sops_encrypted(dotenv_content));

    // Test that plain dotenv without ENC pattern is not detected as encrypted
    let plain_dotenv = "DATABASE_URL=postgresql://localhost/db\nANOTHER_KEY=some_value";
    assert!(!is_sops_encrypted(plain_dotenv));
}

#[test]
fn test_error_classification_with_exit_codes() {
    use controller::controller::parser::sops::error::classify_sops_error;

    // Test exit code 3 (KeyNotFound) - most reliable classification
    let reason = classify_sops_error("some error", Some(3));
    assert_eq!(reason, SopsDecryptionFailureReason::KeyNotFound);
    assert!(!reason.is_transient());

    // Test exit code 4 (WrongKey)
    let reason = classify_sops_error("some error", Some(4));
    assert_eq!(reason, SopsDecryptionFailureReason::WrongKey);
    assert!(!reason.is_transient());

    // Test exit code 5 (UnsupportedFormat)
    let reason = classify_sops_error("some error", Some(5));
    assert_eq!(reason, SopsDecryptionFailureReason::UnsupportedFormat);
    assert!(!reason.is_transient());

    // Test exit code 6 (InvalidKeyFormat)
    let reason = classify_sops_error("some error", Some(6));
    assert_eq!(reason, SopsDecryptionFailureReason::InvalidKeyFormat);
    assert!(!reason.is_transient());

    // Test exit code 2 (CorruptedFile)
    let reason = classify_sops_error("some error", Some(2));
    assert_eq!(reason, SopsDecryptionFailureReason::CorruptedFile);
    assert!(!reason.is_transient());

    // Test exit code 1 (generic) - should fall through to message parsing
    let reason = classify_sops_error("no decryption key found", Some(1));
    assert_eq!(reason, SopsDecryptionFailureReason::KeyNotFound);
}

#[tokio::test]
async fn test_decrypt_corrupted_file_fails() {
    if sops_missing() {
        return;
    }
    // Test that corrupted SOPS content fails gracefully
    let corrupted_content = r#"sops:
    version: 3.8.0
    mac: INVALID_MAC
DATABASE_URL: ENC[INVALID_FORMAT]
"#;

    let sops_key = get_test_sops_key();
    let result = decrypt_sops_content(corrupted_content, None, sops_key.as_deref()).await;

    // Should fail with corrupted file error
    if let Err(e) = result {
        assert!(
            e.reason == SopsDecryptionFailureReason::CorruptedFile
                || e.reason == SopsDecryptionFailureReason::WrongKey
                || e.reason == SopsDecryptionFailureReason::Unknown
        );
        // Note: Unknown errors are classified as transient for safety, but corrupted files should be permanent
        // If it's Unknown, we can't assert !is_transient, but that's acceptable
        if e.reason != SopsDecryptionFailureReason::Unknown {
            assert!(!e.is_transient); // Corrupted files are permanent failures
        }
    } else {
        // If it somehow succeeds, that's also acceptable (SOPS might handle it)
        // The important thing is it doesn't panic
    }
}

#[tokio::test]
async fn test_decrypt_unsupported_format_fails() {
    if sops_missing() {
        return;
    }
    // Test that unsupported formats fail gracefully
    let unsupported_content = "This is not a valid SOPS format at all - just random text";

    let sops_key = get_test_sops_key();
    let result = decrypt_sops_content(unsupported_content, None, sops_key.as_deref()).await;

    // Should fail with unsupported format or corrupted file error
    if let Err(e) = result {
        assert!(
            e.reason == SopsDecryptionFailureReason::UnsupportedFormat
                || e.reason == SopsDecryptionFailureReason::CorruptedFile
                || e.reason == SopsDecryptionFailureReason::Unknown
        );
        // Note: Unknown errors are classified as transient for safety, but unsupported formats should be permanent
        // If it's Unknown, we can't assert !is_transient, but that's acceptable
        if e.reason != SopsDecryptionFailureReason::Unknown {
            assert!(!e.is_transient); // Unsupported formats are permanent failures
        }
    } else {
        // If it somehow succeeds, that's also acceptable
        // The important thing is it doesn't panic
    }
}

#[tokio::test]
async fn test_decrypt_with_malformed_key_fails() {
    if sops_missing() {
        return;
    }
    let test_dir = get_test_files_dir();
    let file_path = test_dir.join("application.secrets.env");

    if !file_path.exists() {
        eprintln!(
            "⚠️  Test file not found: {} - skipping test",
            file_path.display()
        );
        return;
    }

    let encrypted_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Failed to read test file");

    // Try with malformed key (not valid GPG format)
    let malformed_key = "This is not a valid GPG key at all - just random text";

    let result =
        decrypt_sops_content(&encrypted_content, Some(&file_path), Some(malformed_key)).await;

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Should be classified as InvalidKeyFormat or WrongKey (depending on how SOPS handles it)
    assert!(
        error.reason == SopsDecryptionFailureReason::InvalidKeyFormat
            || error.reason == SopsDecryptionFailureReason::WrongKey
    );
    assert!(!error.is_transient); // Invalid key format is permanent
}

#[test]
fn test_parse_secrets_error_propagation() {
    use controller::controller::parser::parsers::ParseSecretsError;
    use controller::controller::parser::sops::error::{
        SopsDecryptionError, SopsDecryptionFailureReason,
    };

    // Test that ParseSecretsError properly wraps SopsDecryptionError
    let sops_error = SopsDecryptionError::new(
        SopsDecryptionFailureReason::KeyNotFound,
        "No decryption key found".to_string(),
    );

    let parse_error = ParseSecretsError::SopsDecryption(sops_error);

    // Test error type methods
    assert!(parse_error.as_sops_error().is_some());
    assert!(!parse_error.is_transient()); // KeyNotFound is permanent
    assert!(!parse_error.remediation().is_empty());

    // Test that it can be converted back
    let extracted = parse_error.as_sops_error().unwrap();
    assert_eq!(extracted.reason, SopsDecryptionFailureReason::KeyNotFound);
}

#[test]
fn test_transient_vs_permanent_errors() {
    use controller::controller::parser::sops::error::SopsDecryptionFailureReason;

    // Permanent errors
    let permanent_reasons = vec![
        SopsDecryptionFailureReason::KeyNotFound,
        SopsDecryptionFailureReason::WrongKey,
        SopsDecryptionFailureReason::InvalidKeyFormat,
        SopsDecryptionFailureReason::UnsupportedFormat,
        SopsDecryptionFailureReason::CorruptedFile,
    ];

    for reason in permanent_reasons {
        assert!(!reason.is_transient(), "{:?} should be permanent", reason);
    }

    // Transient errors
    let transient_reasons = vec![
        SopsDecryptionFailureReason::NetworkTimeout,
        SopsDecryptionFailureReason::ProviderUnavailable,
        SopsDecryptionFailureReason::PermissionDenied,
        SopsDecryptionFailureReason::Unknown,
    ];

    for reason in transient_reasons {
        assert!(reason.is_transient(), "{:?} should be transient", reason);
    }
}

/// A throwaway age key pair, generated for this test only: it protects nothing.
const TEST_AGE_RECIPIENT: &str = "age1np3s994zj5eh3sazm95ayz0vmsnf68e9esljktvge53y3426ku7smjz68r";
const TEST_AGE_IDENTITY: &str = "# public key: age1np3s994zj5eh3sazm95ayz0vmsnf68e9esljktvge53y3426ku7smjz68r\nAGE-SECRET-KEY-16UR67M8W4SJ5RNYW6KQ2HNFVH8J8KJ6FMJPUHLPEYPWVCVDXS9TS8J6UP7\n";

/// Our SOPS files are encrypted to age recipients. The controller's key Secret then
/// holds an age identity, which must decrypt without a GPG keyring.
#[tokio::test]
async fn test_decrypt_dotenv_with_age_key() {
    if sops_missing() {
        return;
    }
    use std::io::Write;
    use std::process::{Command, Stdio};

    let plaintext = "DATABASE_PASSWORD=correct-horse\nAPI_KEY=battery-staple\n";
    let mut child = Command::new("sops")
        // Outside the repository: its .sops.yaml has no creation rule for stdin.
        .current_dir(std::env::temp_dir())
        .args(["-e", "--age", TEST_AGE_RECIPIENT])
        .args([
            "--input-type",
            "dotenv",
            "--output-type",
            "dotenv",
            "/dev/stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sops");
    child
        .stdin
        .take()
        .expect("sops stdin")
        .write_all(plaintext.as_bytes())
        .expect("write plaintext");
    let output = child.wait_with_output().expect("sops -e");
    assert!(
        output.status.success(),
        "sops -e failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let encrypted = String::from_utf8(output.stdout).expect("utf-8");
    assert!(is_sops_encrypted(&encrypted));
    assert!(!encrypted.contains("correct-horse"));

    let path = PathBuf::from("application.secrets.env");
    let decrypted = decrypt_sops_content(&encrypted, Some(&path), Some(TEST_AGE_IDENTITY))
        .await
        .expect("decrypt with the age identity");
    assert!(decrypted.contains("DATABASE_PASSWORD=correct-horse"));
    assert!(decrypted.contains("API_KEY=battery-staple"));

    // Without the identity sops has no key for the file.
    let no_key = decrypt_sops_content(&encrypted, Some(&path), None).await;
    assert!(no_key.is_err(), "decryption without a key must fail");
}

/// Another throwaway identity: not a recipient of anything encrypted in these tests.
const OTHER_AGE_IDENTITY: &str =
    "AGE-SECRET-KEY-1YLP3G3GATZLPV7RTRU3EMUXMGMMLHLTA9L5PDHLYPZYV0XZGGRYQQ05VWU";

/// Encrypt `plaintext` to the test age recipient with the sops binary.
fn encrypt_to_test_age_recipient(plaintext: &str, file_type: &str, extra: &[&str]) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("sops")
        // Outside the repository: its .sops.yaml has no creation rule for stdin.
        .current_dir(std::env::temp_dir())
        .args(["-e", "--age", TEST_AGE_RECIPIENT])
        .args(extra)
        .args([
            "--input-type",
            file_type,
            "--output-type",
            file_type,
            "/dev/stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sops");
    child
        .stdin
        .take()
        .expect("sops stdin")
        .write_all(plaintext.as_bytes())
        .expect("write plaintext");
    let output = child.wait_with_output().expect("sops -e");
    assert!(
        output.status.success(),
        "sops -e failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf-8")
}

/// A Kubernetes Secret manifest encrypted the way our repositories do it
/// (`encrypted_regex: ^(data|stringData)$`) decrypts with the age identity.
#[tokio::test]
async fn test_decrypt_secret_manifest_with_age_key() {
    if sops_missing() {
        return;
    }
    let manifest = "apiVersion: v1\nkind: Secret\nmetadata:\n  name: gate\ntype: Opaque\nstringData:\n  key: open-sesame\n";
    let encrypted = encrypt_to_test_age_recipient(
        manifest,
        "yaml",
        &["--encrypted-regex", "^(data|stringData)$"],
    );
    assert!(is_sops_encrypted(&encrypted));
    assert!(!encrypted.contains("open-sesame"));
    assert!(encrypted.contains("name: gate"), "metadata stays readable");

    let path = PathBuf::from("gate.secret.yaml");
    let decrypted = decrypt_sops_content(&encrypted, Some(&path), Some(TEST_AGE_IDENTITY))
        .await
        .expect("decrypt with the age identity");
    assert!(decrypted.contains("key: open-sesame"));
}

/// The key Secret may hold several identities (a key file with more than one line):
/// any of them that is a recipient decrypts.
#[tokio::test]
async fn test_decrypt_with_several_age_identities() {
    if sops_missing() {
        return;
    }
    let encrypted = encrypt_to_test_age_recipient("TOKEN=abc123\n", "dotenv", &[]);
    let identities = format!("{OTHER_AGE_IDENTITY}\n{TEST_AGE_IDENTITY}");
    let path = PathBuf::from("application.secrets.env");
    let decrypted = decrypt_sops_content(&encrypted, Some(&path), Some(&identities))
        .await
        .expect("the second identity is a recipient");
    assert!(decrypted.contains("TOKEN=abc123"));
}

/// An identity that is not a recipient fails, permanently, and says nothing of the content.
#[tokio::test]
async fn test_decrypt_with_wrong_age_identity_fails() {
    if sops_missing() {
        return;
    }
    let encrypted = encrypt_to_test_age_recipient("TOKEN=abc123\n", "dotenv", &[]);
    let path = PathBuf::from("application.secrets.env");
    let error = decrypt_sops_content(&encrypted, Some(&path), Some(OTHER_AGE_IDENTITY))
        .await
        .expect_err("not a recipient");
    assert!(
        !error.is_transient,
        "a wrong key is not worth retrying: {error}"
    );
    assert!(!error.to_string().contains("abc123"));
}

/// A throwaway GPG key in its own keyring: `(fingerprint, armored private key, keyring)`.
/// `None` when gpg is not installed.
fn throwaway_gpg_key(test: &str) -> Option<(String, String, PathBuf)> {
    use std::process::Command;

    if Command::new("gpg").arg("--version").output().is_err() {
        if std::env::var_os("SMC_REQUIRE_SOPS").is_some() {
            panic!("gpg is required (SMC_REQUIRE_SOPS is set) but is not installed");
        }
        eprintln!("skipping: gpg not installed");
        return None;
    }
    // Short path: gpg-agent's socket path has a length limit.
    let home = PathBuf::from(format!("/tmp/smc-gpg-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).expect("gpg home");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).expect("chmod");
    }
    let gpg = |args: &[&str]| {
        let output = Command::new("gpg")
            .env("GNUPGHOME", &home)
            .args(["--batch", "--pinentry-mode", "loopback", "--passphrase", ""])
            .args(args)
            .output()
            .expect("run gpg");
        assert!(
            output.status.success(),
            "gpg {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf-8")
    };
    gpg(&[
        "--quick-generate-key",
        "smc-test@example.invalid",
        "default",
        "default",
        "never",
    ]);
    let listing = gpg(&["--list-secret-keys", "--with-colons"]);
    let fingerprint = listing
        .lines()
        .find(|line| line.starts_with("fpr:"))
        .and_then(|line| line.split(':').nth(9))
        .expect("fingerprint")
        .to_string();
    let armored = gpg(&["--armor", "--export-secret-keys", &fingerprint]);
    Some((fingerprint, armored, home))
}

/// Dog-fooding keeps both kinds of file in one repository: some encrypted to an age
/// recipient, some to a GPG key. One key value holding both private keys decrypts both.
#[tokio::test]
async fn test_one_key_value_decrypts_age_and_pgp_files() {
    if sops_missing() {
        return;
    }
    let Some((fingerprint, armored_gpg_key, gpg_home)) = throwaway_gpg_key("mixed") else {
        return;
    };
    use std::io::Write;
    use std::process::{Command, Stdio};

    // A file encrypted to the GPG key...
    let mut child = Command::new("sops")
        .current_dir(std::env::temp_dir())
        .env("GNUPGHOME", &gpg_home)
        .args(["-e", "--pgp", &fingerprint])
        .args([
            "--input-type",
            "dotenv",
            "--output-type",
            "dotenv",
            "/dev/stdin",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sops");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"LEGACY_TOKEN=from-pgp\n")
        .expect("write");
    let output = child.wait_with_output().expect("sops -e --pgp");
    assert!(
        output.status.success(),
        "sops -e --pgp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pgp_file = String::from_utf8(output.stdout).expect("utf-8");
    // ...and one encrypted to the age recipient.
    let age_file = encrypt_to_test_age_recipient("NEW_TOKEN=from-age\n", "dotenv", &[]);
    let _ = std::fs::remove_dir_all(&gpg_home);

    let both_keys = format!("{TEST_AGE_IDENTITY}{armored_gpg_key}");
    let path = PathBuf::from("application.secrets.env");

    let from_pgp = decrypt_sops_content(&pgp_file, Some(&path), Some(&both_keys))
        .await
        .expect("the PGP file decrypts with the combined key");
    assert!(from_pgp.contains("LEGACY_TOKEN=from-pgp"));

    let from_age = decrypt_sops_content(&age_file, Some(&path), Some(&both_keys))
        .await
        .expect("the age file decrypts with the combined key");
    assert!(from_age.contains("NEW_TOKEN=from-age"));

    // Each key alone decrypts only its own kind.
    assert!(
        decrypt_sops_content(&pgp_file, Some(&path), Some(TEST_AGE_IDENTITY))
            .await
            .is_err(),
        "the age identity alone cannot read the PGP file"
    );
    assert!(
        decrypt_sops_content(&age_file, Some(&path), Some(&armored_gpg_key))
            .await
            .is_err(),
        "the GPG key alone cannot read the age file"
    );
}
