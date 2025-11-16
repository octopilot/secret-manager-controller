//! # Fibonacci Backoff
//!
//! Provides a Fibonacci-based backoff mechanism for retries.
//! This provides a progressive backoff that grows more slowly than exponential backoff,
//! making it suitable for operations that may need multiple retries without overwhelming the system.
//!
//! The backoff sequence is calculated in minutes to align with GitOps tool conventions.
//! Sequence: 1m, 1m, 2m, 3m, 5m, 8m, 10m (max), then converted to seconds for use in the reconciler.
//!
//! ## Usage
//!
//! ```rust
//! use secret_manager_controller::controller::backoff::FibonacciBackoff;
//!
//! let mut backoff = FibonacciBackoff::new(1, 10); // 1 minute min, 10 minutes max
//! assert_eq!(backoff.next_backoff_seconds(), 60);  // 1m = 60s
//! assert_eq!(backoff.next_backoff_seconds(), 60);  // 1m = 60s
//! assert_eq!(backoff.next_backoff_seconds(), 120); // 2m = 120s
//! assert_eq!(backoff.next_backoff_seconds(), 180); // 3m = 180s
//! assert_eq!(backoff.next_backoff_seconds(), 300); // 5m = 300s
//! ```

use std::time::Duration;

/// Fibonacci backoff calculator
///
/// Generates backoff durations following the Fibonacci sequence.
/// Calculations are performed in minutes (aligning with GitOps tool conventions),
/// then converted to seconds for use in the reconciler.
/// Each backoff is the sum of the previous two backoffs.
///
/// # Example
///
/// ```
/// use secret_manager_controller::controller::backoff::FibonacciBackoff;
///
/// let mut backoff = FibonacciBackoff::new(1, 10); // 1 minute min, 10 minutes max
/// println!("Backoff: {}s", backoff.next_backoff_seconds());
/// ```
#[derive(Debug, Clone)]
pub struct FibonacciBackoff {
    /// Minimum backoff value in minutes (for reset)
    min_minutes: u64,
    /// Previous backoff value in minutes
    prev_minutes: u64,
    /// Current backoff value in minutes
    current_minutes: u64,
    /// Maximum backoff value in minutes
    max_minutes: u64,
}

impl FibonacciBackoff {
    /// Create a new Fibonacci backoff with specified minimum and maximum values in minutes
    ///
    /// Default sequence for reconciliation errors: 1m, 1m, 2m, 3m, 5m, 8m, 10m (max)
    /// Calculations are performed in minutes to align with GitOps tool conventions,
    /// then converted to seconds when returned via `next_backoff_seconds()`.
    ///
    /// # Arguments
    ///
    /// * `min_minutes` - Minimum backoff duration in minutes (used for first two values, typically 1)
    /// * `max_minutes` - Maximum backoff duration in minutes (caps the sequence, typically 10)
    ///
    /// # Example
    ///
    /// ```
    /// use secret_manager_controller::controller::backoff::FibonacciBackoff;
    ///
    /// let backoff = FibonacciBackoff::new(1, 10); // 1 minute min, 10 minutes max
    /// ```
    #[must_use]
    pub fn new(min_minutes: u64, max_minutes: u64) -> Self {
        Self {
            min_minutes,
            prev_minutes: 0,
            current_minutes: min_minutes,
            max_minutes,
        }
    }

    /// Get the next backoff duration in seconds and advance the sequence
    ///
    /// Returns the current backoff value converted from minutes to seconds,
    /// and advances to the next Fibonacci number in minutes.
    /// The sequence is capped at `max_minutes`.
    ///
    /// # Example
    ///
    /// ```
    /// use secret_manager_controller::controller::backoff::FibonacciBackoff;
    ///
    /// let mut backoff = FibonacciBackoff::new(1, 10);
    /// assert_eq!(backoff.next_backoff_seconds(), 60);  // 1m = 60s
    /// assert_eq!(backoff.next_backoff_seconds(), 60);  // 1m = 60s
    /// assert_eq!(backoff.next_backoff_seconds(), 120); // 2m = 120s
    /// ```
    pub fn next_backoff_seconds(&mut self) -> u64 {
        // Convert current minutes to seconds
        let result_seconds = self.current_minutes * 60;

        // Calculate next Fibonacci number in minutes
        let next_minutes = self.prev_minutes + self.current_minutes;

        // Update state (in minutes)
        self.prev_minutes = self.current_minutes;
        self.current_minutes = std::cmp::min(next_minutes, self.max_minutes);

        result_seconds
    }

    /// Get the next backoff duration as a `Duration` and advance the sequence
    ///
    /// # Example
    ///
    /// ```
    /// use secret_manager_controller::controller::backoff::FibonacciBackoff;
    /// use std::time::Duration;
    ///
    /// let mut backoff = FibonacciBackoff::new(1, 60);
    /// assert_eq!(backoff.next_backoff(), Duration::from_secs(1));
    /// ```
    #[must_use]
    pub fn next_backoff(&mut self) -> Duration {
        Duration::from_secs(self.next_backoff_seconds())
    }

    /// Reset the backoff to the initial state
    ///
    /// # Example
    ///
    /// ```
    /// use secret_manager_controller::controller::backoff::FibonacciBackoff;
    ///
    /// let mut backoff = FibonacciBackoff::new(1, 10);
    /// backoff.next_backoff_seconds();
    /// backoff.next_backoff_seconds();
    /// backoff.reset();
    /// assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
    /// ```
    pub fn reset(&mut self) {
        self.prev_minutes = 0;
        self.current_minutes = self.min_minutes;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci_backoff_sequence() {
        let mut backoff = FibonacciBackoff::new(1, 10);

        // Reconciliation error sequence in minutes: 1m, 1m, 2m, 3m, 5m, 8m, 10m (max)
        // Converted to seconds: 60s, 60s, 120s, 180s, 300s, 480s, 600s
        assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
        assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
        assert_eq!(backoff.next_backoff_seconds(), 120); // 2m = 120s
        assert_eq!(backoff.next_backoff_seconds(), 180); // 3m = 180s
        assert_eq!(backoff.next_backoff_seconds(), 300); // 5m = 300s
        assert_eq!(backoff.next_backoff_seconds(), 480); // 8m = 480s
        assert_eq!(backoff.next_backoff_seconds(), 600); // 10m = 600s (max)
    }

    #[test]
    fn test_fibonacci_backoff_max_cap() {
        let mut backoff = FibonacciBackoff::new(1, 10);

        // Should cap at 600 seconds (10 minutes)
        assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
        assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
        assert_eq!(backoff.next_backoff_seconds(), 120); // 2m = 120s
        assert_eq!(backoff.next_backoff_seconds(), 180); // 3m = 180s
        assert_eq!(backoff.next_backoff_seconds(), 300); // 5m = 300s
        assert_eq!(backoff.next_backoff_seconds(), 480); // 8m = 480s
        assert_eq!(backoff.next_backoff_seconds(), 600); // 10m = 600s (max)
                                                         // Next would be 13m (8+5), but should be capped at 10m = 600s
        assert_eq!(backoff.next_backoff_seconds(), 600);
        // Should stay at max
        assert_eq!(backoff.next_backoff_seconds(), 600);
    }

    #[test]
    fn test_fibonacci_backoff_reset() {
        let mut backoff = FibonacciBackoff::new(1, 10);

        assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
        assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
        assert_eq!(backoff.next_backoff_seconds(), 120); // 2m = 120s
        assert_eq!(backoff.next_backoff_seconds(), 180); // 3m = 180s

        backoff.reset();

        // Should restart from beginning after success
        assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
        assert_eq!(backoff.next_backoff_seconds(), 60); // 1m = 60s
        assert_eq!(backoff.next_backoff_seconds(), 120); // 2m = 120s
    }

    #[test]
    fn test_fibonacci_backoff_as_duration() {
        let mut backoff = FibonacciBackoff::new(1, 10);

        assert_eq!(backoff.next_backoff(), Duration::from_secs(60)); // 1m = 60s
        assert_eq!(backoff.next_backoff(), Duration::from_secs(60)); // 1m = 60s
        assert_eq!(backoff.next_backoff(), Duration::from_secs(120)); // 2m = 120s
    }

    #[test]
    fn test_fibonacci_backoff_per_resource_state() {
        // Test that each resource maintains independent backoff state
        // Fibonacci sequence in minutes: 1m, 1m, 2m, 3m, 5m, 8m, 10m...
        // Converted to seconds: 60s, 60s, 120s, 180s, 300s, 480s, 600s...
        let mut backoff1 = FibonacciBackoff::new(1, 10);
        let mut backoff2 = FibonacciBackoff::new(1, 10);

        // Advance first backoff through Fibonacci sequence (in minutes, returned as seconds)
        assert_eq!(backoff1.next_backoff_seconds(), 60); // F(0) = 1m = 60s
        assert_eq!(backoff1.next_backoff_seconds(), 60); // F(1) = 1m = 60s (0+1)
        assert_eq!(backoff1.next_backoff_seconds(), 120); // F(2) = 2m = 120s (1+1)
        assert_eq!(backoff1.next_backoff_seconds(), 180); // F(3) = 3m = 180s (1+2)
        assert_eq!(backoff1.next_backoff_seconds(), 300); // F(4) = 5m = 300s (2+3)

        // Second backoff should start fresh with its own Fibonacci sequence
        assert_eq!(backoff2.next_backoff_seconds(), 60); // F(0) = 1m = 60s
        assert_eq!(backoff2.next_backoff_seconds(), 60); // F(1) = 1m = 60s (0+1)
        assert_eq!(backoff2.next_backoff_seconds(), 120); // F(2) = 2m = 120s (1+1)

        // Reset first backoff (simulating successful reconciliation)
        // This should restart its Fibonacci sequence from the beginning
        backoff1.reset();
        assert_eq!(backoff1.next_backoff_seconds(), 60); // Reset to F(0) = 1m = 60s
        assert_eq!(backoff1.next_backoff_seconds(), 60); // F(1) = 1m = 60s (0+1)
        assert_eq!(backoff1.next_backoff_seconds(), 120); // F(2) = 2m = 120s (1+1)

        // Second backoff continues independently from where it left off
        assert_eq!(backoff2.next_backoff_seconds(), 180); // F(3) = 3m = 180s (1+2)
        assert_eq!(backoff2.next_backoff_seconds(), 300); // F(4) = 5m = 300s (2+3)
    }
}
