//! Tests of the library, on real directories in a temporary folder.

#![allow(clippy::unwrap_used)]

mod covers;
mod meta;
mod ops;
mod scan;
mod section;
mod thumbnail;
mod thumbnail_cache;

use std::fs;
use std::path::Path;

/// Writes `contents` to `root/relative`, creating the folders on the way.
pub(super) fn write(root: &Path, relative: &str, contents: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A library at `root` whose clock always says 1000 ms.
pub(super) fn library_at(root: &Path) -> crate::Library {
    crate::Library::open_with_clock(root, || 1_000).unwrap()
}

/// The paths of the notebooks of `library`, sorted.
pub(super) fn paths(library: &crate::Library) -> Vec<String> {
    let mut paths: Vec<_> = library.notebooks().unwrap().into_iter().map(|n| n.path).collect();
    paths.sort();
    paths
}
