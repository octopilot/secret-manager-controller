//! Unit tests for reconciler module
//!
//! These tests focus on testing individual functions and logic paths
//! without requiring full Kubernetes or cloud provider integration.

#[cfg(test)]
mod tests {
    use secret_manager_controller::controller::reconciler::{
        construct_secret_name, sanitize_secret_name,
    };

    mod secret_name_tests {
        use super::*;

        #[test]
        fn test_construct_secret_name_with_prefix_and_suffix() {
            let result = construct_secret_name(Some("prefix"), "key", Some("suffix"));
            assert_eq!(result, "prefix-key-suffix");
        }

        #[test]
        fn test_construct_secret_name_with_prefix_only() {
            let result = construct_secret_name(Some("prefix"), "key", None);
            assert_eq!(result, "prefix-key");
        }

        #[test]
        fn test_construct_secret_name_with_suffix_only() {
            let result = construct_secret_name(None, "key", Some("suffix"));
            assert_eq!(result, "key-suffix");
        }

        #[test]
        fn test_construct_secret_name_no_prefix_no_suffix() {
            let result = construct_secret_name(None, "key", None);
            assert_eq!(result, "key");
        }

        #[test]
        fn test_sanitize_secret_name_dots() {
            assert_eq!(sanitize_secret_name("test.key"), "test_key");
        }

        #[test]
        fn test_sanitize_secret_name_slashes() {
            assert_eq!(sanitize_secret_name("test/key"), "test_key");
        }

        #[test]
        fn test_sanitize_secret_name_spaces() {
            assert_eq!(sanitize_secret_name("test key"), "test_key");
        }

        #[test]
        fn test_sanitize_secret_name_consecutive_dashes() {
            assert_eq!(sanitize_secret_name("test--key"), "test-key");
        }

        #[test]
        fn test_sanitize_secret_name_leading_trailing_dashes() {
            assert_eq!(sanitize_secret_name("--test--"), "test");
        }

        #[test]
        fn test_sanitize_secret_name_valid_chars() {
            assert_eq!(sanitize_secret_name("test-key_123"), "test-key_123");
        }
    }
}

