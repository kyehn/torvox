//! Mutex lock utilities with poison recovery.
//!
//! # Requirements
//! - FR-049 — Bridge: boltffi ↔ JNA wire format

use std::sync::MutexGuard;

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
