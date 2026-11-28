//! Writing notebooks of the library from an editor: whole or not at all, and never over a
//! version that changed meanwhile.

use std::fs;
use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime};
use sha2::{Digest, Sha256};

use crate::{create_notebook, write_notebook};

/// The time the conflict copies here are named after.
fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 11).unwrap().and_hms_opt(10, 0, 0).unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn write_notebook_writes_when_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rnote"), b"old").unwrap();

    let written =
        write_notebook(dir.path(), "a.rnote", Some(&sha256(b"old")), "Tab", now(), b"new").unwrap();

    assert_eq!(written.path, "a.rnote");
    assert_eq!(written.sha256, sha256(b"new"));
    assert!(!written.conflict);
    assert_eq!(fs::read(dir.path().join("a.rnote")).unwrap(), b"new");
}

#[test]
fn write_notebook_keeps_a_changed_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rnote"), b"from another device").unwrap();

    let written =
        write_notebook(dir.path(), "a.rnote", Some(&sha256(b"old")), "Tab", now(), b"mine")
            .unwrap();

    assert_eq!(written.path, "a (conflicto, Tab, 2026-11-11 10.00).rnote");
    assert!(written.conflict);
    assert_eq!(fs::read(dir.path().join("a.rnote")).unwrap(), b"from another device");
    assert_eq!(fs::read(dir.path().join(&written.path)).unwrap(), b"mine");
}

#[test]
fn write_notebook_creates_a_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("Orga")).unwrap();

    let written =
        write_notebook(dir.path(), "Orga/a.rnote", Some(&sha256(b"old")), "Tab", now(), b"x")
            .unwrap();

    assert_eq!(written.path, "Orga/a.rnote");
    assert!(!written.conflict);
    assert_eq!(fs::read(dir.path().join("Orga/a.rnote")).unwrap(), b"x");
    assert_eq!(fs::read_dir(dir.path().join("Orga")).unwrap().count(), 1);
}

#[test]
fn create_notebook_writes_it_whole_and_leaves_no_temporary() {
    let dir = tempfile::tempdir().unwrap();

    create_notebook(dir.path(), "Orga/a.rnote", b"whole").unwrap();

    assert_eq!(fs::read(dir.path().join("Orga/a.rnote")).unwrap(), b"whole");
    let names: Vec<_> = fs::read_dir(dir.path().join("Orga"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["a.rnote"]);
}

#[test]
fn create_notebook_never_replaces_one() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rnote"), b"mine").unwrap();

    assert!(create_notebook(dir.path(), "a.rnote", b"new").is_err());

    assert_eq!(fs::read(dir.path().join("a.rnote")).unwrap(), b"mine");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn create_notebook_rejects_paths_outside() {
    let dir = tempfile::tempdir().unwrap();

    for path in ["../x.rnote", ".rnote-library/x.rnote", "x.txt"] {
        assert!(create_notebook(dir.path(), path, b"x").is_err(), "{path}");
    }
}

#[test]
fn write_notebook_rejects_paths_outside() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("library");
    fs::create_dir(&root).unwrap();

    for path in ["../x.rnote", "/x.rnote", ".rnote-library/x.rnote", "Orga/../../x.rnote"] {
        assert!(write_notebook(&root, path, None, "Tab", now(), b"x").is_err(), "{path}");
    }
    assert!(!Path::new(&dir.path().join("x.rnote")).exists());
}

#[test]
fn write_notebook_recreates_a_removed_folder() {
    let dir = tempfile::tempdir().unwrap();

    let written =
        write_notebook(dir.path(), "Orga/a.rnote", Some(&sha256(b"old")), "Tab", now(), b"x")
            .unwrap();

    assert_eq!(written.path, "Orga/a.rnote");
    assert_eq!(fs::read(dir.path().join("Orga/a.rnote")).unwrap(), b"x");
}

#[test]
fn write_notebook_waits_for_the_write_guard() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_owned();
    fs::write(root.join("a.rnote"), b"old").unwrap();
    let held = rnote_library::write_guard();

    let writer = std::thread::spawn(move || {
        write_notebook(&root, "a.rnote", Some(&sha256(b"old")), "Tab", now(), b"new").unwrap();
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    let while_held = fs::read(dir.path().join("a.rnote")).unwrap();
    drop(held);
    writer.join().unwrap();

    assert_eq!(while_held, b"old");
    assert_eq!(fs::read(dir.path().join("a.rnote")).unwrap(), b"new");
}

#[test]
fn a_save_after_the_notebook_was_moved_goes_to_its_new_place() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.rnote"), b"old").unwrap();
    fs::create_dir(dir.path().join("F")).unwrap();
    // Moved (by a sync or the library) while the editor still had the old path.
    let (from, to) = (dir.path().join("a.rnote"), dir.path().join("F/b.rnote"));
    fs::rename(&from, &to).unwrap();
    rnote_library::record_move(&from, &to);

    let written =
        write_notebook(dir.path(), "a.rnote", Some(&sha256(b"old")), "Tab", now(), b"new").unwrap();

    assert_eq!(written.path, "F/b.rnote");
    assert!(!written.conflict);
    assert!(!from.exists(), "no notebook comes back at the old place");
    assert_eq!(fs::read(&to).unwrap(), b"new");
}

#[test]
fn a_save_after_its_folder_was_renamed_follows_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("Algebra")).unwrap();
    fs::write(dir.path().join("Algebra/a.rnote"), b"old").unwrap();
    let (from, to) = (dir.path().join("Algebra"), dir.path().join("Álgebra II"));
    fs::rename(&from, &to).unwrap();
    rnote_library::record_move(&from, &to);

    let written =
        write_notebook(dir.path(), "Algebra/a.rnote", Some(&sha256(b"old")), "Tab", now(), b"new")
            .unwrap();

    assert_eq!(written.path, "Álgebra II/a.rnote");
    assert!(!dir.path().join("Algebra").exists());
}
