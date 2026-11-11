use std::fs;

use super::Device;
use crate::Remote;
use crate::fake::FakeDrive;

/// The id of the live file named `name` in `drive`.
fn id_of(drive: &FakeDrive, name: &str) -> String {
    let listing = drive.clone().list().unwrap();
    listing.files.into_iter().find(|file| file.name == name).unwrap().id
}

fn rename(device: &Device, from: &str, to: &str) {
    let target = device.root().join(to);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::rename(device.root().join(from), target).unwrap();
}

#[test]
fn local_rename_moves_the_remote_file_without_upload() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    let id = id_of(&drive, "a.rnote");

    rename(&a, "a.rnote", "Orga/b.rnote");
    let report = a.sync(&drive);

    assert_eq!(drive.paths(), vec!["Orga/b.rnote"]);
    assert_eq!(id_of(&drive, "b.rnote"), id);
    assert!(drive.trashed_paths().is_empty());
    assert_eq!((report.moved, report.uploaded), (1, 0));
}

#[test]
fn remote_move_moves_the_local_file() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("Orga/a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    rename(&a, "Orga/a.rnote", "b.rnote");
    a.sync(&drive);
    let report = b.sync(&drive);

    assert_eq!(b.paths(), vec!["b.rnote"]);
    assert_eq!(b.read("b.rnote"), b"1");
    // A still has the folder it moved the notebook out of, so B keeps it too.
    assert!(a.root().join("Orga").is_dir() && b.root().join("Orga").is_dir());
    assert_eq!((report.moved, report.downloaded), (1, 0));
}

#[test]
fn trash_in_the_app_reaches_the_other_device() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    rnote_library::Library::open(a.root()).unwrap().trash("a.rnote").unwrap();
    a.sync(&drive);
    b.sync(&drive);

    let library = rnote_library::Library::open(b.root()).unwrap();
    let entries = library.trash_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].original_path, "a.rnote");
    assert!(library.notebooks().unwrap().is_empty());
    assert!(drive.trashed_paths().is_empty());
}

#[test]
fn moved_and_edited_on_one_device_arrives_moved_and_edited() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    rename(&a, "a.rnote", "Orga/b.rnote");
    a.write("Orga/b.rnote", b"2");
    a.sync(&drive);
    b.sync(&drive);

    assert_eq!(b.paths(), vec!["Orga/b.rnote"]);
    assert_eq!(b.read("Orga/b.rnote"), b"2");
}

#[test]
fn moved_on_one_device_and_edited_on_the_other_keeps_the_edit_at_the_new_place() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    rename(&a, "a.rnote", "Orga/b.rnote");
    a.sync(&drive);
    b.write("a.rnote", b"2");
    b.sync(&drive);
    a.sync(&drive);

    for device in [&a, &b] {
        assert_eq!(device.paths(), vec!["Orga/b.rnote"]);
        assert_eq!(device.read("Orga/b.rnote"), b"2");
    }
}

#[test]
fn remote_move_onto_a_taken_local_name_keeps_both() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    b.write("Orga/b.rnote", b"mine");
    rename(&a, "a.rnote", "Orga/b.rnote");
    a.sync(&drive);
    b.sync(&drive);

    assert_eq!(b.read("Orga/b.rnote"), b"mine");
    assert_eq!(b.read("Orga/b (2).rnote"), b"1");
    assert_eq!(drive.paths(), vec!["Orga/b (2).rnote", "Orga/b.rnote"]);
}
