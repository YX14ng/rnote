use std::collections::BTreeMap;
use std::fs;

use super::Device;
use crate::SyncError;
use crate::fake::FakeDrive;

type Files = BTreeMap<String, Vec<u8>>;

fn local_files(device: &Device) -> Files {
    device.paths().into_iter().map(|path| (path.clone(), device.read(&path))).collect()
}

/// The live files of `drive` and its trashed paths.
fn drive_files(drive: &FakeDrive) -> (Files, Vec<String>) {
    let paths = drive.paths();
    let mut unique = paths.clone();
    unique.dedup();
    assert_eq!(paths, unique, "two files share a path in Drive");
    let files =
        paths.into_iter().map(|path| (path.clone(), drive.content(&path).unwrap())).collect();
    (files, drive.trashed_paths())
}

/// A synced drive and two devices, then changes on both: B's are synced, A's aren't.
fn scenario() -> (FakeDrive, Device, Device) {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    for (path, bytes) in [
        ("keep.rnote", b"k".as_slice()),
        ("edit.rnote", b"1"),
        ("move.rnote", b"m"),
        ("delete.rnote", b"d"),
        ("Orga/x.rnote", b"x"),
    ] {
        a.write(path, bytes);
    }
    a.sync(&drive);
    b.sync(&drive);
    b.write("from_b.rnote", b"b");
    b.write("Orga/x.rnote", b"x2");
    b.delete("keep.rnote");
    b.sync(&drive);
    a.write("new.rnote", b"n");
    a.write("edit.rnote", b"2");
    fs::create_dir_all(a.root().join("Orga")).unwrap();
    fs::rename(a.root().join("move.rnote"), a.root().join("Orga/moved.rnote")).unwrap();
    a.delete("delete.rnote");
    (drive, a, b)
}

#[test]
fn interrupted_sync_finishes_without_duplicates() {
    let (drive, a, _b) = scenario();
    let before = drive.ops();
    a.sync(&drive);
    let operations = drive.ops() - before;
    let expected = (local_files(&a), drive_files(&drive));
    assert!(operations > 5);

    // Cut before an operation reaches Drive, and right after Drive did it (reply lost).
    for (cut, reply_lost) in (0..operations).flat_map(|cut| [(cut, false), (cut, true)]) {
        let (drive, a, _b) = scenario();
        if reply_lost {
            drive.lose_reply_after(drive.ops() + cut);
        } else {
            drive.fail_after(drive.ops() + cut);
        }
        let at = format!("cut {cut}, reply lost: {reply_lost}");
        assert!(matches!(a.try_sync(&drive), Err(SyncError::Offline(_))), "{at}");
        drive.fail_never();
        a.sync(&drive);

        assert_eq!((local_files(&a), drive_files(&drive)), expected, "{at}");
        let quiet = drive.ops();
        assert_eq!(a.sync(&drive), crate::SyncReport::default(), "{at}");
        assert_eq!(drive.ops(), quiet + 1, "{at}");
    }
}

#[test]
fn three_devices_converge() {
    let drive = FakeDrive::new();
    let devices = [Device::new("A"), Device::new("B"), Device::new("C")];
    devices[0].write("common.rnote", b"0");
    for device in &devices {
        device.sync(&drive);
    }

    for device in &devices {
        device.write(&format!("{}.rnote", device.name), device.name.as_bytes());
        device.write("common.rnote", format!("common {}", device.name).as_bytes());
    }
    for index in [0, 1, 2, 0, 1] {
        devices[index].sync(&drive);
    }

    let (in_drive, _) = drive_files(&drive);
    for device in &devices {
        assert_eq!(local_files(device), in_drive, "device {}", device.name);
    }
    let copies = in_drive.keys().filter(|path| path.contains("conflicto")).count();
    assert_eq!(copies, 2);
    assert_eq!(in_drive.len(), 3 + 1 + copies);
}

#[test]
fn offline_returns_offline_and_changes_nothing_locally() {
    let (drive, a, _b) = scenario();
    let before = local_files(&a);

    drive.fail_after(drive.ops());

    assert!(matches!(a.try_sync(&drive), Err(SyncError::Offline(_))));
    assert_eq!(local_files(&a), before);
}
