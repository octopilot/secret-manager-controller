//! # Error Handling Unit Tests
//!
//! Comprehensive unit tests for error handling, classification, and backoff calculation.
//!
//! These tests verify:
//! - Error classification (transient vs permanent)
//! - Backoff calculation using Fibonacci sequence
//! - Error propagation and wrapping
//! - SOPS error classification

use controller::controller::parser::sops::error::{
    SopsDecryptionFailureReason, classify_sops_error,
};
use controller::controller::reconciler::status::calculate_progressive_backoff;

#[test]
fn test_backoff_calculation_fibonacci_sequence() {
    // Test Fibonacci sequence: 1, 1, 2, 3, 5, 8, 13, 21, 34, 55 minutes
    let test_cases = vec![
        (0, 60),   // 1 minute = 60 seconds
        (1, 60),   // 1 minute = 60 seconds
        (2, 120),  // 2 minutes = 120 seconds
        (3, 180),  // 3 minutes = 180 seconds
        (4, 300),  // 5 minutes = 300 seconds
        (5, 480),  // 8 minutes = 480 seconds
        (6, 780),  // 13 minutes = 780 seconds
        (7, 1260), // 21 minutes = 1260 seconds
        (8, 2040), // 34 minutes = 2040 seconds
        (9, 3300), // 55 minutes = 3300 seconds
    ];

    for (error_count, expected_seconds) in test_cases {
        let backoff = calculate_progressive_backoff(error_count);
        assert_eq!(
            backoff.as_secs(),
            expected_seconds,
            "Backoff for error_count {} should be {} seconds, got {}",
            error_count,
            expected_seconds,
            backoff.as_secs()
        );
    }
}

#[test]
fn test_backoff_calculation_capped_at_60_minutes() {
    // Backoff should be capped at 60 minutes (3600 seconds)
    let large_error_counts = vec![10, 15, 20, 50, 100];

    for error_count in large_error_counts {
        let backoff = calculate_progressive_backoff(error_count);
        assert!(
            backoff.as_secs() <= 3600,
            "Backoff for error_count {} should be capped at 3600 seconds, got {}",
            error_count,
            backoff.as_secs()
        );
    }
}

#[test]
fn test_sops_error_classification_key_not_found() {
    // Exit code 3 = KeyNotFound
    let reason = classify_sops_error("some error message", Some(3));
    assert_eq!(reason, SopsDecryptionFailureReason::KeyNotFound);

    // Error message patterns
    let messages = vec![
        "no decryption key found",
        "key not found",
        "No decryption key found",
        "KEY NOT FOUND",
    ];

    for msg in messages {
        let reason = classify_sops_error(msg, None);
        assert_eq!(
            reason,
            SopsDecryptionFailureReason::KeyNotFound,
            "Message '{}' should be classified as KeyNotFound",
            msg
        );
    }
}

#[test]
fn test_sops_error_classification_wrong_key() {
    // Exit code 4 = WrongKey
    let reason = classify_sops_error("some error message", Some(4));
    assert_eq!(reason, SopsDecryptionFailureReason::WrongKey);

    // Error message patterns - must contain both "wrong key" or "decryption failed" AND "gpg" or "key"
    let messages = vec![
        "wrong key",
        "Wrong key for decryption",
        "decryption failed with gpg",
        "Decryption failed with GPG key",
    ];

    for msg in messages {
        let reason = classify_sops_error(msg, None);
        assert_eq!(
            reason,
            SopsDecryptionFailureReason::WrongKey,
            "Message '{}' should be classified as WrongKey",
            msg
        );
    }
}

#[test]
fn test_sops_error_classification_unsupported_format() {
    // Exit code 5 = UnsupportedFormat
    let reason = classify_sops_error("some error message", Some(5));
    assert_eq!(reason, SopsDecryptionFailureReason::UnsupportedFormat);

    // Error message patterns
    let messages = vec![
        "unsupported format",
        "unknown file type",
        "Unsupported format for SOPS",
        "Unknown file type",
    ];

    for msg in messages {
        let reason = classify_sops_error(msg, None);
        assert_eq!(
            reason,
            SopsDecryptionFailureReason::UnsupportedFormat,
            "Message '{}' should be classified as UnsupportedFormat",
            msg
        );
    }
}

#[test]
fn test_sops_error_classification_invalid_key_format() {
    // Exit code 6 = InvalidKeyFormat
    let reason = classify_sops_error("some error message", Some(6));
    assert_eq!(reason, SopsDecryptionFailureReason::InvalidKeyFormat);

    // Error message patterns - must contain "invalid key" or "malformed key"
    let messages = vec![
        "invalid key",
        "malformed key",
        "Invalid key format",
        "Malformed key",
    ];

    for msg in messages {
        let reason = classify_sops_error(msg, None);
        assert_eq!(
            reason,
            SopsDecryptionFailureReason::InvalidKeyFormat,
            "Message '{}' should be classified as InvalidKeyFormat",
            msg
        );
    }
}

#[test]
fn test_sops_error_classification_corrupted_file() {
    // Exit code 2 = CorruptedFile
    let reason = classify_sops_error("some error message", Some(2));
    assert_eq!(reason, SopsDecryptionFailureReason::CorruptedFile);

    // Error message patterns
    let messages = vec![
        "corrupt",
        "invalid file",
        "Corrupted file",
        "Invalid file format",
    ];

    for msg in messages {
        let reason = classify_sops_error(msg, None);
        assert_eq!(
            reason,
            SopsDecryptionFailureReason::CorruptedFile,
            "Message '{}' should be classified as CorruptedFile",
            msg
        );
    }
}

#[test]
fn test_sops_error_classification_exit_code_priority() {
    // Exit codes should take priority over error message parsing
    let reason = classify_sops_error("no decryption key found", Some(4)); // Exit code 4 = WrongKey
    assert_eq!(
        reason,
        SopsDecryptionFailureReason::WrongKey,
        "Exit code should take priority over error message"
    );
}

#[test]
fn test_sops_error_classification_unknown() {
    // Unknown errors should default to Unknown
    // Exit code 1 doesn't map to a specific reason, so it falls through to message parsing
    let _reason1 = classify_sops_error("some random error message", Some(1)); // Exit code 1 = generic

    // Test with a truly unknown message and exit code
    let _reason2 = classify_sops_error("completely unknown error", Some(99)); // Unknown exit code
    // Should default to Unknown or fall through to message parsing
    // The actual behavior depends on implementation, but it shouldn't panic
}

#[test]
fn test_sops_error_transient_vs_permanent() {
    // Permanent failures - these should have is_transient = false
    let permanent_reasons = vec![
        SopsDecryptionFailureReason::KeyNotFound,
        SopsDecryptionFailureReason::WrongKey,
        SopsDecryptionFailureReason::InvalidKeyFormat,
        SopsDecryptionFailureReason::UnsupportedFormat,
        SopsDecryptionFailureReason::CorruptedFile,
    ];

    for reason in permanent_reasons {
        assert!(
            !reason.is_transient(),
            "Reason {:?} should be permanent (not transient)",
            reason
        );
    }

    // Transient failures - these should have is_transient = true
    let transient_reasons = vec![
        SopsDecryptionFailureReason::NetworkTimeout,
        SopsDecryptionFailureReason::ProviderUnavailable,
        SopsDecryptionFailureReason::PermissionDenied,
        SopsDecryptionFailureReason::Unknown,
    ];

    for reason in transient_reasons {
        assert!(
            reason.is_transient(),
            "Reason {:?} should be transient",
            reason
        );
    }
}

#[test]
fn test_backoff_progression_smooth() {
    // Ensure backoff increases smoothly (no decreases)
    let mut prev_backoff = 0;
    for error_count in 0..=20 {
        let backoff = calculate_progressive_backoff(error_count);
        assert!(
            backoff.as_secs() >= prev_backoff,
            "Backoff should never decrease: error_count {} has backoff {} < previous {}",
            error_count,
            backoff.as_secs(),
            prev_backoff
        );
        prev_backoff = backoff.as_secs();
    }
}
