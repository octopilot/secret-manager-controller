//! # Fibonacci Backoff
//!
//! Provides a Fibonacci-based backoff mechanism for retries.
//! This provides a progressive backoff that grows more slowly than exponential backoff,
//! making it suitable for operations that may need multiple retries without overwhelming the system.
//!
//! ## Usage
//!
//! ```rust
//! use secret_manager_controller::controller::backoff::FibonacciBackoff;
//!
//! let mut backoff = FibonacciBackoff::new(1, 60);
//! assert_eq!(backoff.next_backoff_seconds(), 1);
//! assert_eq!(backoff.next_backoff_seconds(), 1);
//! assert_eq!(backoff.next_backoff_seconds(), 2);
//! assert_eq!(backoff.next_backoff_seconds(), 3);
//! assert_eq!(backoff.next_backoff_seconds(), 5);
//! ```

use std::time::Duration;

/// Fibonacci backoff calculator
/// 
/// Generates backoff durations following the Fibonacci sequence.
/// Each backoff is the sum of the previous two backoffs.
/// 
/// # Example
/// 
/// ```
/// use secret_manager_controller::controller::backoff::FibonacciBackoff;
/// 
/// let mut backoff = FibonacciBackoff::new(1, 300);
/// println!("Backoff: {}s", backoff.next_backoff_seconds());
/// ```
#[derive(Debug, Clone)]
pub struct FibonacciBackoff {
    /// Minimum backoff value in seconds (for reset)
    min_seconds: u64,
    /// Previous backoff value in seconds
    prev: u64,
    /// Current backoff value in seconds
    current: u64,
    /// Maximum backoff value in seconds
    max_seconds: u64,
}

impl FibonacciBackoff {
    /// Create a new Fibonacci backoff with specified minimum and maximum values
    /// 
    /// # Arguments
    /// 
    /// * `min_seconds` - Minimum backoff duration in seconds (used for first two values)
    /// * `max_seconds` - Maximum backoff duration in seconds (caps the sequence)
    /// 
    /// # Example
    /// 
    /// ```
    /// use secret_manager_controller::controller::backoff::FibonacciBackoff;
    /// 
    /// let backoff = FibonacciBackoff::new(1, 300);
    /// ```
    #[must_use]
    pub fn new(min_seconds: u64, max_seconds: u64) -> Self {
        Self {
            min_seconds,
            prev: 0,
            current: min_seconds,
            max_seconds,
        }
    }

    /// Get the next backoff duration and advance the sequence
    /// 
    /// Returns the current backoff value and advances to the next Fibonacci number.
    /// The sequence is capped at `max_seconds`.
    /// 
    /// # Example
    /// 
    /// ```
    /// use secret_manager_controller::controller::backoff::FibonacciBackoff;
    /// 
    /// let mut backoff = FibonacciBackoff::new(1, 60);
    /// assert_eq!(backoff.next_backoff_seconds(), 1);
    /// assert_eq!(backoff.next_backoff_seconds(), 1);
    /// assert_eq!(backoff.next_backoff_seconds(), 2);
    /// ```
    pub fn next_backoff_seconds(&mut self) -> u64 {
        let result = self.current;
        
        // Calculate next Fibonacci number
        let next = self.prev + self.current;
        
        // Update state
        self.prev = self.current;
        self.current = std::cmp::min(next, self.max_seconds);
        
        result
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
    /// let mut backoff = FibonacciBackoff::new(1, 60);
    /// backoff.next_backoff_seconds();
    /// backoff.next_backoff_seconds();
    /// backoff.reset();
    /// assert_eq!(backoff.next_backoff_seconds(), 1);
    /// ```
    pub fn reset(&mut self) {
        self.prev = 0;
        self.current = self.min_seconds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci_backoff_sequence() {
        let mut backoff = FibonacciBackoff::new(1, 300);
        
        // First few Fibonacci numbers starting with 1
        assert_eq!(backoff.next_backoff_seconds(), 1);
        assert_eq!(backoff.next_backoff_seconds(), 1);
        assert_eq!(backoff.next_backoff_seconds(), 2);
        assert_eq!(backoff.next_backoff_seconds(), 3);
        assert_eq!(backoff.next_backoff_seconds(), 5);
        assert_eq!(backoff.next_backoff_seconds(), 8);
        assert_eq!(backoff.next_backoff_seconds(), 13);
        assert_eq!(backoff.next_backoff_seconds(), 21);
        assert_eq!(backoff.next_backoff_seconds(), 34);
        assert_eq!(backoff.next_backoff_seconds(), 55);
        assert_eq!(backoff.next_backoff_seconds(), 89);
        assert_eq!(backoff.next_backoff_seconds(), 144);
        assert_eq!(backoff.next_backoff_seconds(), 233);
    }

    #[test]
    fn test_fibonacci_backoff_max_cap() {
        let mut backoff = FibonacciBackoff::new(1, 60);
        
        // Should cap at 60 seconds
        assert_eq!(backoff.next_backoff_seconds(), 1);
        assert_eq!(backoff.next_backoff_seconds(), 1);
        assert_eq!(backoff.next_backoff_seconds(), 2);
        assert_eq!(backoff.next_backoff_seconds(), 3);
        assert_eq!(backoff.next_backoff_seconds(), 5);
        assert_eq!(backoff.next_backoff_seconds(), 8);
        assert_eq!(backoff.next_backoff_seconds(), 13);
        assert_eq!(backoff.next_backoff_seconds(), 21);
        assert_eq!(backoff.next_backoff_seconds(), 34);
        assert_eq!(backoff.next_backoff_seconds(), 55);
        // Next would be 89, but should be capped at 60
        assert_eq!(backoff.next_backoff_seconds(), 60);
        // Should stay at max
        assert_eq!(backoff.next_backoff_seconds(), 60);
    }

    #[test]
    fn test_fibonacci_backoff_reset() {
        let mut backoff = FibonacciBackoff::new(2, 100);
        
        assert_eq!(backoff.next_backoff_seconds(), 2);
        assert_eq!(backoff.next_backoff_seconds(), 2);
        assert_eq!(backoff.next_backoff_seconds(), 4);
        
        backoff.reset();
        
        // Should restart from beginning
        assert_eq!(backoff.next_backoff_seconds(), 2);
        assert_eq!(backoff.next_backoff_seconds(), 2);
        assert_eq!(backoff.next_backoff_seconds(), 4);
    }

    #[test]
    fn test_fibonacci_backoff_as_duration() {
        let mut backoff = FibonacciBackoff::new(5, 100);
        
        assert_eq!(backoff.next_backoff(), Duration::from_secs(5));
        assert_eq!(backoff.next_backoff(), Duration::from_secs(5));
        assert_eq!(backoff.next_backoff(), Duration::from_secs(10));
    }

    #[test]
    fn test_fibonacci_backoff_different_min() {
        let mut backoff = FibonacciBackoff::new(10, 300);
        
        assert_eq!(backoff.next_backoff_seconds(), 10);
        assert_eq!(backoff.next_backoff_seconds(), 10);
        assert_eq!(backoff.next_backoff_seconds(), 20);
        assert_eq!(backoff.next_backoff_seconds(), 30);
        assert_eq!(backoff.next_backoff_seconds(), 50);
    }
}
