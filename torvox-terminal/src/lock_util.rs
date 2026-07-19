//! Mutex lock utilities with poison recovery.
//!
//! # Requirements
//! - FR-049 — Bridge: boltffi ↔ JNA wire format

use std::sync::MutexGuard;

/// Lock a mutex, recovering from poisoning if necessary.
///
/// If the mutex is poisoned (a previous holder panicked), this logs a warning
/// and returns the inner value anyway, rather than panicking. This is the
/// standard recovery pattern for cross-thread mutexes in torvox.
pub(crate) fn lock_or_recover<'a, T>(
    mutex: &'a std::sync::Mutex<T>,
    context: &str,
) -> MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("{context}: mutex poisoned, recovered");
            poisoned.into_inner()
        }
    }
}
