//! # Pending retries
//!
//! A reconcile that fails, or that has to wait for something (a source that does not
//! exist yet, a transient provider error), asks to be run again after a delay. The
//! watch loop only reconciles an object when it has a reason to: its spec changed, its
//! interval is due, its source has a new artifact. A retry is one more such reason, and
//! this registry is where it is recorded, so the run that arrives when the delay is over
//! is recognised as the retry and not skipped as "nothing changed".
//!
//! The registry is in memory: after a restart every object is reconciled anyway.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// A run that arrives this much before the retry is due still counts as the retry:
/// timers and requeues are not exact.
const TOLERANCE: Duration = Duration::from_secs(2);

fn pending() -> &'static Mutex<HashMap<String, Instant>> {
    static PENDING: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(namespace: &str, name: &str) -> String {
    format!("{namespace}/{name}")
}

/// Record that `namespace/name` is to be reconciled again after `delay`.
pub fn schedule(namespace: &str, name: &str, delay: Duration) {
    if let Ok(mut pending) = pending().lock() {
        pending.insert(key(namespace, name), Instant::now() + delay);
    }
}

/// True, once, when a retry of `namespace/name` is pending and due.
pub fn take_if_due(namespace: &str, name: &str) -> bool {
    let Ok(mut pending) = pending().lock() else {
        return false;
    };
    let key = key(namespace, name);
    let due = pending
        .get(&key)
        .is_some_and(|due_at| Instant::now() + TOLERANCE >= *due_at);
    if due {
        pending.remove(&key);
    }
    due
}

/// How long until the pending retry of `namespace/name` is due, if there is one.
pub fn time_until(namespace: &str, name: &str) -> Option<Duration> {
    let pending = pending().lock().ok()?;
    pending
        .get(&key(namespace, name))
        .map(|due_at| due_at.saturating_duration_since(Instant::now()))
}

/// Forget the pending retry of `namespace/name`: it is being reconciled now, or is gone.
pub fn clear(namespace: &str, name: &str) {
    if let Ok(mut pending) = pending().lock() {
        pending.remove(&key(namespace, name));
    }
}

/// Delay before retrying after the `attempt`th consecutive transient failure:
/// 30s, 1m, 2m, 4m, 8m, then 10m.
#[must_use]
pub fn transient_delay(attempt: u32) -> Duration {
    let doublings = attempt.saturating_sub(1).min(5);
    Duration::from_secs((30u64 << doublings).min(600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_retry_is_not_due_before_its_delay() {
        schedule("retry-ns", "later", Duration::from_secs(300));
        assert!(!take_if_due("retry-ns", "later"));
        let remaining = time_until("retry-ns", "later").expect("pending");
        assert!(remaining > Duration::from_secs(290) && remaining <= Duration::from_secs(300));
        clear("retry-ns", "later");
        assert_eq!(time_until("retry-ns", "later"), None);
    }

    #[test]
    fn a_due_retry_is_taken_once() {
        schedule("retry-ns", "now", Duration::from_secs(1));
        assert!(take_if_due("retry-ns", "now"), "within the tolerance");
        assert!(!take_if_due("retry-ns", "now"), "taken");
    }

    #[test]
    fn nothing_pending_is_not_a_retry() {
        assert!(!take_if_due("retry-ns", "never-scheduled"));
        assert_eq!(time_until("retry-ns", "never-scheduled"), None);
    }

    #[test]
    fn a_new_schedule_replaces_the_old_one() {
        schedule("retry-ns", "replaced", Duration::from_secs(600));
        schedule("retry-ns", "replaced", Duration::from_secs(1));
        assert!(take_if_due("retry-ns", "replaced"));
    }

    #[test]
    fn transient_delay_grows_to_ten_minutes() {
        let seconds: Vec<u64> = (0..=8).map(|n| transient_delay(n).as_secs()).collect();
        assert_eq!(seconds, vec![30, 30, 60, 120, 240, 480, 600, 600, 600]);
    }
}
