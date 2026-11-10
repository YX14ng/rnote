use std::fs;

use super::write;
use crate::names::{unique_file_name, validate_name};
use crate::{Library, NotebookKind};

#[test]
fn lists_rnote_and_xopp_in_root_and_one_folder_level() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.rnote", b"a");
    write(root, "b.xopp", b"b");
    write(root, "F/c.rnote", b"c");
    write(root, "F/G/d.rnote", b"d");
    write(root, "notas.txt", b"t");
    write(root, ".x.autosave.xopp", b"x");

    let library = Library::open(root).unwrap();
    let mut found: Vec<_> = library
        .notebooks()
        .unwrap()
        .into_iter()
        .map(|n| (n.path, n.name, n.folder, n.kind))
        .collect();
    found.sort();

    assert_eq!(
        found,
        vec![
            ("F/c.rnote".to_owned(), "c".to_owned(), Some("F".to_owned()), NotebookKind::Rnote),
            ("a.rnote".to_owned(), "a".to_owned(), None, NotebookKind::Rnote),
            ("b.xopp".to_owned(), "b".to_owned(), None, NotebookKind::Xopp),
        ]
    );
}

#[test]
fn notebooks_report_size_and_modification_time() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"12345");

    let notebooks = Library::open(dir.path()).unwrap().notebooks().unwrap();

    assert_eq!(notebooks[0].size, 5);
    assert!(notebooks[0].modified_ms > 0);
}

#[test]
fn open_creates_the_library_dir_without_touching_notebooks() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"12345");

    Library::open(dir.path()).unwrap();

    assert!(dir.path().join(".rnote-library").is_dir());
    assert_eq!(fs::read(dir.path().join("a.rnote")).unwrap(), b"12345");
}

#[test]
fn folders_lists_first_level_dirs_except_hidden() {
    let dir = tempfile::tempdir().unwrap();
    for folder in ["F", "Álgebra", ".git"] {
        fs::create_dir(dir.path().join(folder)).unwrap();
    }

    let mut folders = Library::open(dir.path()).unwrap().folders().unwrap();
    folders.sort();

    assert_eq!(folders, vec!["F".to_owned(), "Álgebra".to_owned()]);
}

#[test]
fn validate_name_rejects_and_trims() {
    assert_eq!(validate_name("  hola  ").unwrap(), "hola");
    for bad in ["a/b", "a\\b", ".x", "   ", "", "a\0b"] {
        assert!(validate_name(bad).is_err(), "{bad:?} should be rejected");
    }
    assert_eq!(validate_name("Análisis 😀").unwrap(), "Análisis 😀");
    assert_eq!(validate_name(&"a".repeat(200)).unwrap().chars().count(), 120);
}

#[test]
fn unique_file_name_appends_counter() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(unique_file_name(dir.path(), "x", "rnote"), "x.rnote");
    write(dir.path(), "x.rnote", b"");
    write(dir.path(), "x (2).rnote", b"");

    assert_eq!(unique_file_name(dir.path(), "x", "rnote"), "x (3).rnote");
}
