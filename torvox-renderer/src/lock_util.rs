#[cfg(target_os = "android")]
use std::sync::{RwLock, RwLockWriteGuard};

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
