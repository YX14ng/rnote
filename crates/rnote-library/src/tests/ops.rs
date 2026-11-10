use std::fs;

use super::{library_at, paths, write};

#[test]
fn rename_and_move_keep_tags() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    let mut library = library_at(dir.path());
    let tag = library.create_tag("Parcial", 0xFFFF_0000).unwrap();
    library.set_tags("a.rnote", std::slice::from_ref(&tag)).unwrap();
    library.create_folder("F").unwrap();

    let renamed = library.rename("a.rnote", "b").unwrap();
    let moved = library.move_to(&renamed, Some("F")).unwrap();

    assert_eq!(moved, "F/b.rnote");
    let notebooks = library.notebooks().unwrap();
    assert_eq!(notebooks.len(), 1);
    assert_eq!(notebooks[0].path, "F/b.rnote");
    assert_eq!(notebooks[0].tags, vec![tag]);
}

#[test]
fn rename_rejects_invalid_names_and_taken_ones() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    write(dir.path(), "b.rnote", b"b");
    let mut library = library_at(dir.path());

    assert!(library.rename("a.rnote", "x/y").is_err());
    assert!(library.rename("a.rnote", "b").is_err());
    assert_eq!(fs::read(dir.path().join("b.rnote")).unwrap(), b"b");
}

#[test]
fn trash_and_restore_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "F/b.rnote", b"b");
    let mut library = library_at(dir.path());

    library.trash("F/b.rnote").unwrap();

    assert!(paths(&library).is_empty());
    let entries = library.trash_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].original_path, "F/b.rnote");
    assert_eq!(entries[0].name, "b");
    assert_eq!(entries[0].deleted_at_ms, 1_000);

    let restored = library.restore(&entries[0].id).unwrap();

    assert_eq!(restored, "F/b.rnote");
    assert_eq!(paths(&library), vec!["F/b.rnote"]);
    assert!(library.trash_entries().is_empty());
}

#[test]
fn restore_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "F/b.rnote", b"old");
    let mut library = library_at(dir.path());
    library.trash("F/b.rnote").unwrap();
    write(dir.path(), "F/b.rnote", b"new");
    let id = library.trash_entries()[0].id.clone();

    let restored = library.restore(&id).unwrap();

    assert_eq!(restored, "F/b (2).rnote");
    assert_eq!(fs::read(dir.path().join("F/b.rnote")).unwrap(), b"new");
    assert_eq!(fs::read(dir.path().join("F/b (2).rnote")).unwrap(), b"old");
}

#[test]
fn restore_recreates_a_deleted_folder() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "F/b.rnote", b"b");
    let mut library = library_at(dir.path());
    library.trash("F/b.rnote").unwrap();
    fs::remove_dir(dir.path().join("F")).unwrap();
    let id = library.trash_entries()[0].id.clone();

    assert_eq!(library.restore(&id).unwrap(), "F/b.rnote");
}

#[test]
fn delete_folder_trashes_its_notebooks() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "F/a.rnote", b"a");
    write(dir.path(), "F/b.xopp", b"b");
    let mut library = library_at(dir.path());

    library.delete_folder("F").unwrap();

    assert!(!dir.path().join("F").exists());
    assert!(library.folders().unwrap().is_empty());
    let mut originals: Vec<_> =
        library.trash_entries().into_iter().map(|e| e.original_path).collect();
    originals.sort();
    assert_eq!(originals, vec!["F/a.rnote", "F/b.xopp"]);
}

#[test]
fn rename_folder_moves_its_notebooks_and_tags() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "F/a.rnote", b"a");
    let mut library = library_at(dir.path());
    let tag = library.create_tag("t", 1).unwrap();
    library.set_tags("F/a.rnote", std::slice::from_ref(&tag)).unwrap();

    assert_eq!(library.rename_folder("F", "G").unwrap(), "G");

    let notebooks = library.notebooks().unwrap();
    assert_eq!(notebooks[0].path, "G/a.rnote");
    assert_eq!(notebooks[0].tags, vec![tag]);
}

#[test]
fn delete_forever_and_empty_trash_remove_files() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    write(dir.path(), "b.rnote", b"b");
    write(dir.path(), "c.rnote", b"c");
    let mut library = library_at(dir.path());
    for path in ["a.rnote", "b.rnote", "c.rnote"] {
        library.trash(path).unwrap();
    }
    let first = library.trash_entries()[0].id.clone();

    library.delete_forever(&first).unwrap();
    assert_eq!(library.trash_entries().len(), 2);

    library.empty_trash().unwrap();
    assert!(library.trash_entries().is_empty());
    let trash_dir = dir.path().join(".rnote-library/trash");
    assert_eq!(fs::read_dir(trash_dir).unwrap().count(), 0);
}

#[test]
fn duplicate_copies_bytes_and_tags() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"bytes");
    let mut library = library_at(dir.path());
    let tag = library.create_tag("t", 1).unwrap();
    library.set_tags("a.rnote", std::slice::from_ref(&tag)).unwrap();

    let copy = library.duplicate("a.rnote").unwrap();

    assert_eq!(copy, "a (2).rnote");
    assert_eq!(fs::read(dir.path().join(&copy)).unwrap(), b"bytes");
    let tags_of_copy =
        library.notebooks().unwrap().into_iter().find(|n| n.path == copy).unwrap().tags;
    assert_eq!(tags_of_copy, vec![tag]);
}

#[test]
fn new_notebook_path_is_free_and_not_created() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "F/Sin título.rnote", b"");
    let library = library_at(dir.path());

    let path = library.new_notebook_path(Some("F"), "Sin título").unwrap();

    assert_eq!(path, "F/Sin título (2).rnote");
    assert!(!dir.path().join(&path).exists());
}

#[test]
fn create_folder_rejects_taken_and_invalid_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut library = library_at(dir.path());

    assert_eq!(library.create_folder(" Álgebra ").unwrap(), "Álgebra");
    assert!(library.create_folder("Álgebra").is_err());
    assert!(library.create_folder(".oculta").is_err());
}

#[test]
fn operations_reject_paths_outside_the_library() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    let mut library = library_at(dir.path());

    assert!(library.trash("../a.rnote").is_err());
    assert!(library.rename("/etc/passwd", "x").is_err());
    assert!(library.move_to("a.rnote", Some("..")).is_err());
}

#[test]
fn rename_changing_only_case_works() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "algebra.rnote", b"a");
    let mut library = library_at(dir.path());

    assert_eq!(library.rename("algebra.rnote", "Algebra").unwrap(), "Algebra.rnote");
    assert_eq!(paths(&library), vec!["Algebra.rnote"]);
}

#[test]
fn free_path_keeps_the_extension_and_avoids_taken_names() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "F/x.xopp", b"");
    let library = library_at(dir.path());

    assert_eq!(library.free_path(Some("F"), "x.xopp").unwrap(), "F/x (2).xopp");
    assert_eq!(library.free_path(None, "y.rnote").unwrap(), "y.rnote");
    assert!(library.free_path(None, "../y.rnote").is_err());
}
