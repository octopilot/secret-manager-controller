//! # Duration Validation
//!
//! Handles parsing and validating Kubernetes duration strings.

use anyhow::Result;
use regex::Regex;
use std::time::Duration;

/// Parse Kubernetes duration string into std::time::Duration
/// Supports formats: "30s", "1m", "5m", "1h", "2h", "1d"
/// Returns Duration or error if format is invalid
pub fn parse_kubernetes_duration(duration_str: &str) -> Result<Duration> {
    let duration_trimmed = duration_str.trim();

    if duration_trimmed.is_empty() {
        return Err(anyhow::anyhow!("Duration string cannot be empty"));
    }

    // Regex pattern for Kubernetes duration format
    // Matches: <number><unit> where:
    //   - number: one or more digits
    //   - unit: s, m, h, d (case insensitive)
    let duration_regex = Regex::new(r"^(?P<number>\d+)(?P<unit>[smhd])$")
        .map_err(|e| anyhow::anyhow!("Failed to compile regex: {e}"))?;

    // Match against trimmed, lowercase version
    let interval_lower = duration_trimmed.to_lowercase();

    let captures = duration_regex
        .captures(&interval_lower)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid duration format '{}'. Expected format: <number><unit> (e.g., '1m', '5m', '1h')",
                duration_trimmed
            )
        })?;

    // Extract number and unit from regex captures
    let number_str = captures
        .name("number")
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to extract number from duration '{}'",
                duration_trimmed
            )
        })?
        .as_str();

    let unit = captures
        .name("unit")
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Failed to extract unit from duration '{}'",
                duration_trimmed
            )
        })?
        .as_str();

    // Parse number safely
    let number: u64 = number_str.parse().map_err(|e| {
        anyhow::anyhow!(
            "Invalid duration number '{}' in '{}': {}",
            number_str,
            duration_trimmed,
            e
        )
    })?;

    if number == 0 {
        return Err(anyhow::anyhow!(
            "Duration number must be greater than 0, got '{}'",
            duration_trimmed
        ));
    }

    // Convert to seconds based on unit
    let seconds = match unit {
        "s" => number,
        "m" => number * 60,
        "h" => number * 3600,
        "d" => number * 86400,
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid unit '{}' in duration '{}'. Expected: s, m, h, or d",
                unit,
                duration_trimmed
            ));
        }
    };

    Ok(Duration::from_secs(seconds))
}

/// Validate duration interval with regex and minimum value check
/// Ensures interval matches Kubernetes duration format and meets minimum requirement
/// Accepts Kubernetes duration format: "1m", "5m", "1h", etc.
///
/// # Arguments
/// * `interval` - The duration string to validate
/// * `field_name` - The name of the field being validated (for error messages)
/// * `min_seconds` - Minimum duration in seconds
///
/// # Returns
/// Ok(()) if valid, Err with descriptive message if invalid
pub fn validate_duration_interval(
    interval: &str,
    field_name: &str,
    min_seconds: u64,
) -> Result<()> {
    // Trim whitespace
    let interval_trimmed = interval.trim();

    if interval_trimmed.is_empty() {
        return Err(anyhow::anyhow!("{field_name} cannot be empty"));
    }

    // Parse the duration to validate format
    let duration = parse_kubernetes_duration(interval_trimmed)?;

    // Check minimum duration
    if duration.as_secs() < min_seconds {
        return Err(anyhow::anyhow!(
            "{field_name} '{}' must be at least {} seconds (got {} seconds)",
            interval_trimmed,
            min_seconds,
            duration.as_secs()
        ));
    }

    Ok(())
}
