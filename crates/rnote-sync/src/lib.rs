//! Syncs an Rnote library folder with a folder named `Rnote` in Google Drive.
//!
//! Nothing is ever lost: when a notebook changed on both sides, both versions are kept.
//! Drive is reached through the [`Remote`] trait, so the whole algorithm is tested against
//! an in-memory Drive.

#[cfg(any(test, feature = "fake"))]
pub mod fake;
// Used by `sync()` from the next commit on; until then only the tests use them.
#[allow(dead_code)]
mod local;
#[allow(dead_code)]
mod lock;
#[allow(dead_code)]
mod names;
mod remote;
#[allow(dead_code)]
mod state;

#[cfg(test)]
mod tests;

pub use crate::remote::{Listing, ROOT_FOLDER_NAME, Remote, RemoteError, RemoteFile, RemoteFolder};

/// `bytes` in lower-case hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
