use std::fs;

use futures::executor::block_on;
use rnote_engine::Engine;
use rnote_engine::engine::EngineSnapshot;
use rnote_engine::engine::export::{DocExportFormat, DocExportPrefs};

use super::{library_at, write};
use crate::{Notebook, NotebookKind, render_thumbnail, thumbnail_key};

fn empty_document_bytes() -> Vec<u8> {
    block_on(Engine::default().save_as_rnote_bytes("empty".to_owned())).unwrap().unwrap()
}

fn notebook(modified_ms: i64) -> Notebook {
    Notebook {
        path: "a.rnote".to_owned(),
        name: "a".to_owned(),
        folder: None,
        kind: NotebookKind::Rnote,
        modified_ms,
        size: 10,
        tags: Vec::new(),
        cover: None,
    }
}

#[test]
fn thumbnail_of_a_new_document_is_a_png() {
    let png = render_thumbnail(empty_document_bytes(), 256).unwrap();

    assert!(png.starts_with(b"\x89PNG"));
    let image = image::load_from_memory(&png).unwrap();
    assert!(image.width() <= 256 && image.height() <= 256);
    assert!(image.width() == 256 || image.height() == 256);
}

#[test]
fn thumbnail_of_garbage_is_an_error() {
    assert!(render_thumbnail(vec![1, 2, 3], 256).is_err());
}

#[test]
fn thumbnail_key_changes_with_mtime() {
    let key = thumbnail_key(&notebook(1));

    assert_eq!(key, thumbnail_key(&notebook(1)));
    assert_ne!(key, thumbnail_key(&notebook(2)));
}

#[test]
fn thumbnail_key_is_the_same_whatever_rust_built_it() {
    // FNV-1a of the path, then of the time and size: a new compiler keeps the cache valid.
    assert_eq!(thumbnail_key(&notebook(1)), "07075dd8540e8c48-6eedd04bbb43316e");
}

#[test]
fn thumbnail_looks_like_the_page_drawn_through_svg() {
    let bytes = include_bytes!("../../../../misc/file-tests/v0-15-0-test.rnote").to_vec();

    let direct =
        image::load_from_memory(&render_thumbnail(bytes.clone(), 256).unwrap()).unwrap().to_rgba8();
    let through_svg = image::load_from_memory(
        &crate::thumbnail::render_thumbnail_through_svg(bytes, 256).unwrap(),
    )
    .unwrap()
    .to_rgba8();

    assert_eq!(direct.dimensions(), through_svg.dimensions());
    let total: u64 = direct
        .as_raw()
        .iter()
        .zip(through_svg.as_raw())
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    let mean = total as f64 / direct.as_raw().len() as f64;
    assert!(mean < 3.0, "mean difference {mean}");
}

#[test]
fn import_xopp_writes_a_new_rnote_beside_it() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = DocExportPrefs { export_format: DocExportFormat::Xopp, ..Default::default() };
    let xopp =
        block_on(Engine::default().export_doc("x".to_owned(), Some(prefs))).unwrap().unwrap();
    write(dir.path(), "F/x.xopp", &xopp);
    let mut library = library_at(dir.path());

    let imported = library.import_xopp("F/x.xopp").unwrap();

    assert_eq!(imported, "F/x.rnote");
    assert_eq!(fs::read(dir.path().join("F/x.xopp")).unwrap(), xopp);
    let bytes = fs::read(dir.path().join(&imported)).unwrap();
    assert!(block_on(EngineSnapshot::load_from_rnote_bytes(bytes)).is_ok());
}

#[test]
fn import_xopp_never_overwrites_an_rnote() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = DocExportPrefs { export_format: DocExportFormat::Xopp, ..Default::default() };
    let xopp =
        block_on(Engine::default().export_doc("x".to_owned(), Some(prefs))).unwrap().unwrap();
    write(dir.path(), "x.xopp", &xopp);
    write(dir.path(), "x.rnote", b"mine");
    let mut library = library_at(dir.path());

    assert_eq!(library.import_xopp("x.xopp").unwrap(), "x (2).rnote");
    assert_eq!(fs::read(dir.path().join("x.rnote")).unwrap(), b"mine");
}

#[test]
fn import_xopp_gives_the_new_rnote_the_tags_of_the_xopp() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = DocExportPrefs { export_format: DocExportFormat::Xopp, ..Default::default() };
    let xopp =
        block_on(Engine::default().export_doc("x".to_owned(), Some(prefs))).unwrap().unwrap();
    write(dir.path(), "x.xopp", &xopp);
    let mut library = library_at(dir.path());
    let tag = library.create_tag("Parcial", 0xFFFF_0000).unwrap();
    library.set_tags("x.xopp", std::slice::from_ref(&tag)).unwrap();

    let imported = library.import_xopp("x.xopp").unwrap();

    let notebooks = library.notebooks().unwrap();
    let rnote = notebooks.iter().find(|notebook| notebook.path == imported).unwrap();
    assert_eq!(rnote.tags, vec![tag.clone()]);
    let reopened = library_at(dir.path());
    let notebooks = reopened.notebooks().unwrap();
    let rnote = notebooks.iter().find(|notebook| notebook.path == imported).unwrap();
    assert_eq!(rnote.tags, vec![tag]);
}
