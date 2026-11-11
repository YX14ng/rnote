//! One sync at a time: a lock on a file beside the state, which the system releases if the
//! process dies.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

use anyhow::Context;

/// Held while a sync runs; dropping it lets the next one in.
#[derive(Debug)]
pub(crate) struct SyncLock(#[allow(dead_code)] File);

/// Takes the lock of the state at `state_path`, or `None` if a sync already holds it.
pub(crate) fn acquire(state_path: &Path) -> anyhow::Result<Option<SyncLock>> {
    if let Some(dir) = state_path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("Creating {}", dir.display()))?;
    }
    let mut name = state_path.as_os_str().to_owned();
    name.push(".lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&name)
        .with_context(|| format!("Opening {}", Path::new(&name).display()))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(SyncLock(file))),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error).context("Locking the sync"),
    }
}
