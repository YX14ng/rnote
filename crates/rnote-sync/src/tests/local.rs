use chrono::NaiveDate;

use super::write;
use crate::local::{scan, sha256_hex, write_atomic};
use crate::lock::acquire;
use crate::names::{conflict_name, free_path};
use crate::state::{Entry, SyncState};

fn scanned_paths(root: &std::path::Path) -> Vec<String> {
    scan(root).unwrap().into_keys().collect()
}

#[test]
fn scan_takes_notebooks_library_json_and_trash() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    write(dir.path(), "Orga/b.rnote", b"b");
    write(dir.path(), ".rnote-library/library.json", b"{}");
    write(dir.path(), ".rnote-library/trash/1-a.rnote", b"t");

    let scanned = scan(dir.path()).unwrap();

    assert_eq!(
        scanned.keys().collect::<Vec<_>>(),
        vec![
            ".rnote-library/library.json",
            ".rnote-library/trash/1-a.rnote",
            "Orga/b.rnote",
            "a.rnote"
        ]
    );
    assert_eq!(scanned["a.rnote"], sha256_hex(b"a"));
}

#[test]
fn scan_skips_hidden_temporary_and_other_files() {
    let dir = tempfile::tempdir().unwrap();
    for path in [
        ".Orga.autosave.xopp",
        "Parciales.xopp",
        ".rnote-library/library.json.tmp",
        ".rnote-library/library.json.bad",
        ".rnote-library/other.rnote",
        ".rnote-library/trash/.x.rnote",
        ".a.rnote.sync-tmp",
        ".oculta/x.rnote",
        "Orga/.y.rnote",
        "notas.txt",
    ] {
        write(dir.path(), path, b"x");
    }

    assert!(scanned_paths(dir.path()).is_empty());
}

#[test]
fn scan_accepts_upper_case_extension() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "A.RNOTE", b"a");

    assert_eq!(scanned_paths(dir.path()), vec!["A.RNOTE"]);
}

#[test]
fn write_atomic_creates_folders_and_leaves_no_temporary() {
    let dir = tempfile::tempdir().unwrap();

    write_atomic(dir.path(), "Orga/a.rnote", b"1").unwrap();
    write_atomic(dir.path(), "Orga/a.rnote", b"2").unwrap();

    assert_eq!(std::fs::read(dir.path().join("Orga/a.rnote")).unwrap(), b"2");
    assert_eq!(std::fs::read_dir(dir.path().join("Orga")).unwrap().count(), 1);
}

#[test]
fn sha256_of_abc() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn conflict_name_has_device_and_date() {
    let now = NaiveDate::from_ymd_opt(2026, 11, 10).unwrap().and_hms_opt(21, 50, 0).unwrap();

    assert_eq!(
        conflict_name("Orga/Parcial.rnote", "Galaxy Tab S10", now),
        "Orga/Parcial (conflicto, Galaxy Tab S10, 2026-11-10 21.50).rnote"
    );
    assert_eq!(
        conflict_name("a.rnote", "a/b:c\\d", now),
        "a (conflicto, a-b-c-d, 2026-11-10 21.50).rnote"
    );
}

#[test]
fn free_path_adds_a_number_ignoring_case() {
    let taken =
        |path: &str| ["orga/x.rnote", "orga/x (2).rnote"].contains(&path.to_lowercase().as_str());

    assert_eq!(free_path("Orga/X.rnote", &taken), "Orga/X (3).rnote");
    assert_eq!(free_path("Orga/Y.rnote", &taken), "Orga/Y.rnote");
}

#[test]
fn state_round_trips_and_unreadable_state_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sync/state.json");
    let mut state = SyncState { root_id: Some("root".to_owned()), ..SyncState::default() };
    state.entries.insert(
        "a.rnote".to_owned(),
        Entry {
            id: "1".to_owned(),
            md5: "m".to_owned(),
            modified: "t".to_owned(),
            sha256: "s".to_owned(),
        },
    );

    state.save(&path).unwrap();
    assert_eq!(SyncState::load(&path).unwrap(), state);

    std::fs::write(&path, b"{").unwrap();
    assert_eq!(SyncState::load(&path).unwrap(), SyncState::default());
    assert!(dir.path().join("sync/state.json.bad").exists());
    assert_eq!(SyncState::load(&dir.path().join("missing.json")).unwrap(), SyncState::default());
}

#[test]
fn second_lock_is_refused_until_the_first_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");

    let first = acquire(&path).unwrap();
    assert!(first.is_some());
    assert!(acquire(&path).unwrap().is_none());
    drop(first);
    assert!(acquire(&path).unwrap().is_some());
}
