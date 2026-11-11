use super::{Device, write};
use crate::fake::FakeDrive;
use crate::{Remote, SyncError};

#[test]
fn new_local_notebook_is_uploaded_into_its_folder() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    a.write("Orga/a.rnote", b"a");
    a.write("b.rnote", b"b");

    let report = a.sync(&drive);

    assert_eq!(drive.paths(), vec!["Orga/a.rnote", "b.rnote"]);
    assert_eq!(drive.content("Orga/a.rnote").unwrap(), b"a");
    assert_eq!(report.uploaded, 2);
}

#[test]
fn new_remote_notebook_is_downloaded() {
    let drive = FakeDrive::new();
    drive.add_file("Orga/a.rnote", b"a");
    let b = Device::new("B");

    let report = b.sync(&drive);

    assert_eq!(b.read("Orga/a.rnote"), b"a");
    assert_eq!(report.downloaded, 1);
}

#[test]
fn local_edit_is_uploaded() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    a.write("a.rnote", b"1");
    a.sync(&drive);

    a.write("a.rnote", b"2");
    a.sync(&drive);

    assert_eq!(drive.content("a.rnote").unwrap(), b"2");
    assert_eq!(drive.paths(), vec!["a.rnote"]);
}

#[test]
fn remote_edit_is_downloaded() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    a.write("a.rnote", b"2");
    a.sync(&drive);
    b.sync(&drive);

    assert_eq!(b.read("a.rnote"), b"2");
}

#[test]
fn nothing_changed_does_no_operation() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    a.write("Orga/a.rnote", b"1");
    a.sync(&drive);
    let before = drive.ops();

    let report = a.sync(&drive);

    assert_eq!(drive.ops(), before + 1);
    assert_eq!(report, crate::SyncReport::default());
}

#[test]
fn local_delete_trashes_the_remote_file() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    a.write("a.rnote", b"1");
    a.sync(&drive);

    a.delete("a.rnote");
    let report = a.sync(&drive);

    assert!(drive.paths().is_empty());
    assert_eq!(drive.trashed_paths(), vec!["a.rnote"]);
    assert_eq!(report.deleted, 1);
}

#[test]
fn remote_delete_removes_the_unchanged_local_file() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    a.delete("a.rnote");
    a.sync(&drive);
    b.sync(&drive);

    assert!(b.paths().is_empty());
}

#[test]
fn remote_delete_of_a_locally_edited_file_restores_it() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    a.delete("a.rnote");
    a.sync(&drive);
    b.write("a.rnote", b"2");
    b.sync(&drive);
    a.sync(&drive);

    assert_eq!(drive.content("a.rnote").unwrap(), b"2");
    assert_eq!(a.read("a.rnote"), b"2");
}

#[test]
fn local_delete_of_a_remotely_edited_file_downloads_it() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    b.write("a.rnote", b"2");
    b.sync(&drive);
    a.delete("a.rnote");
    a.sync(&drive);

    assert_eq!(a.read("a.rnote"), b"2");
    assert_eq!(drive.paths(), vec!["a.rnote"]);
}

#[test]
fn empty_remote_folder_is_trashed_and_local_folder_removed() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("Orga/a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    std::fs::remove_dir_all(a.root().join("Orga")).unwrap();
    a.sync(&drive);
    b.sync(&drive);

    let listing = drive.clone().list().unwrap();
    assert!(listing.folders.iter().all(|folder| folder.name != "Orga"));
    assert!(!b.root().join("Orga").exists());
}

#[test]
fn empty_local_folder_is_kept() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    std::fs::create_dir(a.root().join("Nueva")).unwrap();

    a.sync(&drive);

    assert!(a.root().join("Nueva").is_dir());
}

#[test]
fn deleted_root_folder_never_deletes_local_notebooks() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    a.write("a.rnote", b"a");
    a.write("Orga/b.rnote", b"b");
    a.sync(&drive);

    drive.trash_root();
    a.sync(&drive);

    assert_eq!(a.read("a.rnote"), b"a");
    assert_eq!(a.read("Orga/b.rnote"), b"b");
    assert_eq!(drive.paths(), vec!["Orga/b.rnote", "a.rnote"]);
}

#[test]
fn skipped_files_are_never_uploaded() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    for path in
        [".Orga.autosave.xopp", "Parciales.xopp", ".rnote-library/library.json.tmp", "notas.txt"]
    {
        a.write(path, b"x");
    }
    a.write("a.rnote", b"a");
    a.write(".rnote-library/library.json", b"{}");

    a.sync(&drive);

    assert_eq!(drive.paths(), vec![".rnote-library/library.json", "a.rnote"]);
}

#[test]
fn a_second_device_gets_everything() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"a");
    a.write("Orga/b.rnote", b"b");
    a.write(".rnote-library/library.json", b"{}");
    a.write(".rnote-library/trash/1-c.rnote", b"c");
    a.sync(&drive);

    b.sync(&drive);

    assert_eq!(b.paths(), a.paths());
    write(b.root(), "x.rnote", b"x");
    assert_eq!(b.sync(&drive).uploaded, 1);
}

#[test]
fn busy_lock_returns_busy() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let _held = crate::lock::acquire(&a.state).unwrap().unwrap();

    assert!(matches!(a.try_sync(&drive), Err(SyncError::Busy)));
}
