//! # Backoff Calculation
//!
//! Calculates progressive backoff durations for error retries.

/// Calculate progressive backoff duration based on error count
/// Uses Fibonacci sequence to gradually increase retry intervals
/// This prevents controller overload when parsing errors occur
/// Each resource maintains its own error count independently
pub fn calculate_progressive_backoff(error_count: u32) -> std::time::Duration {
    // Fibonacci sequence for backoff (in minutes): 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, then cap at 60
    let fib_sequence = [
        1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610, 987, 1597, 2584, 4181, 6765,
    ]; // in minutes
    let index = std::cmp::min(error_count as usize, fib_sequence.len() - 1);
    let minutes = fib_sequence[index];
    let duration = std::time::Duration::from_secs(minutes * 60); // Convert minutes to seconds

    // Cap at 60 minutes (3600 seconds)
    std::cmp::min(duration, std::time::Duration::from_secs(3600))
}
