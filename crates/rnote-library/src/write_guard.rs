//! One writer at a time for the library's files in this process: the app's own saves, the
//! library's metadata and the sync each check a file and then replace it, and none of them
//! may slip in between another one's check and its write.

use std::sync::{Mutex, MutexGuard, PoisonError};

static GUARD: Mutex<()> = Mutex::new(());

/// Held while checking and then changing files of a library; never held across the network.
pub fn write_guard() -> MutexGuard<'static, ()> {
    GUARD.lock().unwrap_or_else(PoisonError::into_inner)
}
