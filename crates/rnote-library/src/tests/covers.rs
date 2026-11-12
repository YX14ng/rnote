use futures::executor::block_on;
use rnote_engine::Engine;
use rnote_engine::engine::export::{DocExportFormat, DocExportPrefs};
use serde_json::Value;

use super::{library_at, write};

const BLUE: u32 = 0xFF3D_6FB6;

fn cover_of(library: &crate::Library, path: &str) -> Option<u32> {
    library.notebooks().unwrap().into_iter().find(|notebook| notebook.path == path).unwrap().cover
}

fn library_json(root: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join(".rnote-library/library.json")).unwrap())
        .unwrap()
}

#[test]
fn set_cover_shows_on_the_notebook() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "A/x.rnote", b"x");
    let mut library = library_at(dir.path());

    library.set_cover("A/x.rnote", Some(BLUE)).unwrap();
    assert_eq!(cover_of(&library, "A/x.rnote"), Some(BLUE));
    assert_eq!(cover_of(&library_at(dir.path()), "A/x.rnote"), Some(BLUE));

    library.set_cover("A/x.rnote", None).unwrap();
    assert_eq!(cover_of(&library, "A/x.rnote"), None);
}

#[test]
fn set_cover_of_a_missing_notebook_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut library = library_at(dir.path());

    assert!(library.set_cover("nope.rnote", Some(BLUE)).is_err());
}

#[test]
fn cover_follows_rename_move_folder_rename_and_trash() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    let mut library = library_at(dir.path());
    library.set_cover("a.rnote", Some(BLUE)).unwrap();
    library.create_folder("F").unwrap();

    let renamed = library.rename("a.rnote", "b").unwrap();
    let moved = library.move_to(&renamed, Some("F")).unwrap();
    library.rename_folder("F", "G").unwrap();
    assert_eq!(cover_of(&library, "G/b.rnote"), Some(BLUE));
    assert_eq!(moved, "F/b.rnote");

    library.trash("G/b.rnote").unwrap();
    let id = library.trash_entries()[0].id.clone();
    let restored = library.restore(&id).unwrap();

    assert_eq!(cover_of(&library, &restored), Some(BLUE));
}

#[test]
fn duplicate_and_xopp_import_copy_the_cover() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    let prefs = DocExportPrefs { export_format: DocExportFormat::Xopp, ..Default::default() };
    let xopp =
        block_on(Engine::default().export_doc("x".to_owned(), Some(prefs))).unwrap().unwrap();
    write(dir.path(), "x.xopp", &xopp);
    let mut library = library_at(dir.path());
    library.set_cover("a.rnote", Some(BLUE)).unwrap();
    library.set_cover("x.xopp", Some(BLUE)).unwrap();

    let copy = library.duplicate("a.rnote").unwrap();
    let imported = library.import_xopp("x.xopp").unwrap();

    assert_eq!(cover_of(&library, &copy), Some(BLUE));
    assert_eq!(cover_of(&library, &imported), Some(BLUE));
}

#[test]
fn delete_forever_leaves_a_tombstone() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    let mut library = library_at(dir.path());
    library.set_cover("a.rnote", Some(BLUE)).unwrap();
    library.trash("a.rnote").unwrap();
    let id = library.trash_entries()[0].id.clone();

    library.delete_forever(&id).unwrap();

    let json = library_json(dir.path());
    let covers = json["notebook_covers"].as_object().unwrap();
    assert!(!covers.is_empty());
    assert!(covers.values().all(|record| record["deleted"] == true), "{covers:?}");
}
