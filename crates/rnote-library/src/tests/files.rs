//! Files written whole: imported, duplicated and converted notebooks.

use std::fs;

use super::{library_at, paths, write};
use crate::write_new_file;

/// The names in `dir`, hidden ones included.
fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_new_file_is_written_whole_and_never_replaces_one() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a.rnote");

    write_new_file(&target, b"first").unwrap();
    assert!(write_new_file(&target, b"second").is_err());

    assert_eq!(fs::read(&target).unwrap(), b"first");
    assert_eq!(names(dir.path()), ["a.rnote"]);
}

#[test]
fn importing_copies_any_notebook_under_a_free_name() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Orga.xopp", b"old");
    let mut library = library_at(dir.path());

    let xopp = library.import_notebook(None, "Orga.xopp", b"new").unwrap();
    let upper = library.import_notebook(Some("F"), "Parcial.RNOTE", b"rnote").unwrap();

    assert_eq!(xopp, "Orga (2).xopp");
    assert_eq!(upper, "F/Parcial.RNOTE");
    assert_eq!(fs::read(dir.path().join("Orga.xopp")).unwrap(), b"old");
    assert_eq!(fs::read(dir.path().join(&xopp)).unwrap(), b"new");
    assert_eq!(paths(&library), ["F/Parcial.RNOTE", "Orga (2).xopp", "Orga.xopp"]);
}

#[test]
fn importing_something_that_is_not_a_notebook_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut library = library_at(dir.path());

    assert!(library.import_notebook(None, "notas.txt", b"x").is_err());
    assert!(paths(&library).is_empty());
}

#[test]
fn duplicating_leaves_no_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    let mut library = library_at(dir.path());

    let copy = library.duplicate("a.rnote").unwrap();

    assert_eq!(fs::read(dir.path().join(&copy)).unwrap(), b"a");
    assert_eq!(names(dir.path()), [".rnote-library", "a (2).rnote", "a.rnote"]);
}

#[test]
fn replacing_leaves_the_new_content_and_no_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a.rnote");
    let temporary = dir.path().join(".a.rnote.write-tmp");
    std::fs::write(&target, b"old").unwrap();

    crate::replace_durably(&temporary, &target, b"new").unwrap();

    assert_eq!(std::fs::read(&target).unwrap(), b"new");
    assert!(!temporary.exists());
}

#[test]
fn a_failed_replace_leaves_no_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    // A folder with something in it can't be replaced by a file.
    let target = dir.path().join("a.rnote");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("inside"), b"x").unwrap();
    let temporary = dir.path().join(".a.rnote.write-tmp");

    assert!(crate::replace_durably(&temporary, &target, b"new").is_err());

    assert!(!temporary.exists());
}
