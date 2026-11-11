//! Syncs an Rnote library folder with a folder named `Rnote` in Google Drive.
//!
//! Nothing is ever lost: when a notebook changed on both sides, both versions are kept.
//! Drive is reached through the [`Remote`] trait, so the whole algorithm is tested against
//! an in-memory Drive.

#[cfg(any(test, feature = "fake"))]
pub mod fake;
mod remote;

#[cfg(test)]
mod tests;

pub use crate::remote::{Listing, ROOT_FOLDER_NAME, Remote, RemoteError, RemoteFile, RemoteFolder};
