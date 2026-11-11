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

    assert_eq!(key.len(), 16);
    assert_eq!(key, thumbnail_key(&notebook(1)));
    assert_ne!(key, thumbnail_key(&notebook(2)));
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
