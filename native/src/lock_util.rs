//! Merged lock utilities — poison recovery for both `Mutex` and `RwLock`.
//!
//! # Poison recovery strategy
//!
//! Rust's `std::sync::Mutex` and `RwLock` are **poisoned** when a thread
//! panics while holding the lock.  Our response depends on the context:
//!
//! | Context | Strategy | Rationale |
//! |---------|----------|-----------|
//! | Internal state (lock_or_recover, write_or_recover) | **Recover** — log warning, keep the data | Poison means another thread crashed; the lock guard's value is still valid.  Panicking again would mask the original crash. |
//! | JNI FFI (ffi.rs `Session` lock) | **Throw Java exception** — return error to Kotlin | Poison in JNI means the Rust session crashed.  The Kotlin UI must know so it can recreate the session. |
//! | Test assertion | **Panic** — let it propagate | Test failures should be loud. |
//!
//! The split is intentional: `lock_or_recover` is used for internal data
//! structures (EVENT_QUEUE, MCP state) where silent recovery is safe.
//! JNI session access uses `lock()` + `match` + `throw_new()` because the
//! caller (Kotlin) needs to know about the crash.

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn lock_or_recover_normal() {
        let m = Mutex::new(42);
        let guard = lock_or_recover(&m, "test");
        assert_eq!(*guard, 42);
    }

    #[test]
    fn lock_or_recover_after_poison() {
        let m = Mutex::new(7);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = m.lock().unwrap();
            panic!("intentional panic");
        }));
        let guard = lock_or_recover(&m, "test");
        assert_eq!(*guard, 7);
    }

    #[test]
    fn lock_or_recover_written_value_persists() {
        let m = Mutex::new(0);
        {
            let mut g = m.lock().unwrap();
            *g = 99;
        }
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = m.lock().unwrap();
            panic!("intentional panic");
        }));
        let guard = lock_or_recover(&m, "test");
        assert_eq!(*guard, 99);
    }
}
