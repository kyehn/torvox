//! Merged lock utilities — poison recovery for both `Mutex` and `RwLock`.
//!
//! # Requirements
//! - FR-049 — Bridge: boltffi ↔ JNA wire format (Mutex recovery)
//! - FR-010 — GPU renderer resource management (RwLock recovery)

use std::sync::{Mutex, MutexGuard};

#[cfg(target_os = "android")]
use std::sync::{RwLock, RwLockWriteGuard};

/// Lock a mutex, recovering from poisoning if necessary.
///
/// If the mutex is poisoned (a previous holder panicked), this logs a warning
/// and returns the inner value anyway, rather than panicking.
pub(crate) fn lock_or_recover<'a, T>(mutex: &'a Mutex<T>, context: &str) -> MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("{context}: mutex poisoned, recovered");
            poisoned.into_inner()
        }
    }
}

/// Lock a RwLock for writing, recovering from poisoning if necessary.
/// Only used on Android where font DB loading is gated.
#[cfg(target_os = "android")]
pub(crate) fn write_or_recover<'a, T>(
    lock: &'a RwLock<T>,
    context: &str,
) -> RwLockWriteGuard<'a, T> {
    match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => {
            log::warn!("{context}: RwLock poisoned, recovered");
            poisoned.into_inner()
        }
    }
}
