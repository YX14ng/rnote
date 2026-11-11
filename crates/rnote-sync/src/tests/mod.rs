//! Tests of the sync, against the in-memory Drive and real folders in a temporary folder.

#![allow(clippy::unwrap_used)]

mod fake;
mod local;

use std::fs;
use std::path::Path;

/// Writes `contents` to `root/relative`, creating the folders on the way.
pub(super) fn write(root: &Path, relative: &str, contents: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}
