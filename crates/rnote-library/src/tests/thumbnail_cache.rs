use std::fs;
use std::path::Path;

use futures::executor::block_on;
use rnote_engine::Engine;

use super::{library_at, write};
use crate::{Notebook, ThumbnailCache, thumbnail_key};

fn empty_document_bytes() -> Vec<u8> {
    block_on(Engine::default().save_as_rnote_bytes("empty".to_owned())).unwrap().unwrap()
}

/// The library's notebook at `path`, as the scan sees it.
fn scanned(root: &Path, path: &str) -> Notebook {
    library_at(root).notebooks().unwrap().into_iter().find(|n| n.path == path).unwrap()
}

#[test]
fn renders_once_and_then_reads_the_cache() {
    let root = tempfile::tempdir().unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    write(root.path(), "a.rnote", &empty_document_bytes());
    let notebook = scanned(root.path(), "a.rnote");
    let cache = ThumbnailCache::new(cache_dir.path().to_owned());

    let first = cache.get_or_render(root.path(), &notebook, 64).unwrap();
    let png = cache_dir.path().join(format!("{}-64.png", thumbnail_key(&notebook)));
    let mtime = fs::metadata(&png).unwrap().modified().unwrap();
    // Garbage in the notebook proves the second call doesn't render again.
    fs::write(root.path().join("a.rnote"), b"garbage").unwrap();
    let second = cache.get_or_render(root.path(), &notebook, 64).unwrap();

    assert!(first.starts_with(b"\x89PNG"));
    assert_eq!(first, second);
    assert_eq!(fs::metadata(&png).unwrap().modified().unwrap(), mtime);
}

#[test]
fn a_modified_notebook_gets_a_new_thumbnail() {
    let root = tempfile::tempdir().unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    write(root.path(), "a.rnote", &empty_document_bytes());
    let cache = ThumbnailCache::new(cache_dir.path().to_owned());
    let before = scanned(root.path(), "a.rnote");
    cache.get_or_render(root.path(), &before, 64).unwrap();

    let mut after = before.clone();
    after.modified_ms += 1_000;
    cache.get_or_render(root.path(), &after, 64).unwrap();

    assert_ne!(thumbnail_key(&before), thumbnail_key(&after));
    assert!(cache_dir.path().join(format!("{}-64.png", thumbnail_key(&after))).exists());
    assert_eq!(fs::read_dir(cache_dir.path()).unwrap().count(), 2);
}

#[test]
fn different_sizes_are_cached_apart() {
    let root = tempfile::tempdir().unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    write(root.path(), "a.rnote", &empty_document_bytes());
    let notebook = scanned(root.path(), "a.rnote");
    let cache = ThumbnailCache::new(cache_dir.path().to_owned());

    let small = cache.get_or_render(root.path(), &notebook, 32).unwrap();
    let large = cache.get_or_render(root.path(), &notebook, 64).unwrap();

    assert_ne!(small, large);
    let key = thumbnail_key(&notebook);
    assert!(cache_dir.path().join(format!("{key}-32.png")).exists());
    assert!(cache_dir.path().join(format!("{key}-64.png")).exists());
}

#[test]
fn xopp_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    write(root.path(), "x.xopp", b"not rendered");
    let notebook = scanned(root.path(), "x.xopp");
    let cache = ThumbnailCache::new(cache_dir.path().to_owned());

    assert!(cache.get_or_render(root.path(), &notebook, 64).is_err());
    assert_eq!(fs::read_dir(cache_dir.path()).unwrap().count(), 0);
}

#[test]
fn a_missing_cache_folder_is_created() {
    let root = tempfile::tempdir().unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    write(root.path(), "a.rnote", &empty_document_bytes());
    let notebook = scanned(root.path(), "a.rnote");
    let cache = ThumbnailCache::new(cache_dir.path().join("rnote/thumbnails"));

    assert!(cache.get_or_render(root.path(), &notebook, 32).is_ok());
}
