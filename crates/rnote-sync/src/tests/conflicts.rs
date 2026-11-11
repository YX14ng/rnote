use super::Device;
use crate::fake::FakeDrive;

const COPY_A: &str = "a (conflicto, A, 2026-11-10 21.50).rnote";
const COPY_B: &str = "a (conflicto, B, 2026-11-10 21.50).rnote";

fn tag_names(device: &Device) -> Vec<String> {
    let library = rnote_library::Library::open(device.root()).unwrap();
    let mut names: Vec<String> = library.tags().into_iter().map(|tag| tag.name).collect();
    names.sort();
    names
}

#[test]
fn both_edited_keeps_both_versions_everywhere() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);

    a.write("a.rnote", b"2");
    b.write("a.rnote", b"3");
    a.sync(&drive);
    let report = b.sync(&drive);
    a.sync(&drive);

    assert_eq!(report.conflicts, vec![COPY_B]);
    for device in [&a, &b] {
        assert_eq!(device.read("a.rnote"), b"3");
        assert_eq!(device.read(COPY_B), b"2");
    }
    assert_eq!(drive.paths(), vec![COPY_B, "a.rnote"]);
    assert_eq!(drive.content(COPY_B).unwrap(), b"2");
}

#[test]
fn same_name_created_offline_on_two_devices_keeps_both() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    b.write("a.rnote", b"2");

    a.sync(&drive);
    b.sync(&drive);
    a.sync(&drive);

    for device in [&a, &b] {
        assert_eq!(device.read("a.rnote"), b"2");
        assert_eq!(device.read(COPY_B), b"1");
    }
    assert_eq!(drive.paths(), vec![COPY_B, "a.rnote"]);
}

#[test]
fn same_content_on_both_sides_is_not_a_conflict() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    b.write("a.rnote", b"1");

    a.sync(&drive);
    let report = b.sync(&drive);

    assert!(report.conflicts.is_empty());
    assert_eq!(drive.paths(), vec!["a.rnote"]);
}

#[test]
fn two_remote_files_with_one_name_end_up_on_every_device() {
    let drive = FakeDrive::new();
    drive.add_file("a.rnote", b"1");
    drive.add_file("a.rnote", b"2");
    let a = Device::new("A");
    let b = Device::new("B");

    let report = a.sync(&drive);
    b.sync(&drive);

    assert_eq!(report.conflicts, vec![COPY_A]);
    assert_eq!(drive.paths(), vec![COPY_A, "a.rnote"]);
    for device in [&a, &b] {
        assert_eq!(device.read("a.rnote"), b"1");
        assert_eq!(device.read(COPY_A), b"2");
    }
}

#[test]
fn identical_remote_duplicates_are_trashed() {
    let drive = FakeDrive::new();
    drive.add_file("a.rnote", b"1");
    drive.add_file("a.rnote", b"1");
    let a = Device::new("A");

    a.sync(&drive);

    assert_eq!(drive.paths(), vec!["a.rnote"]);
    assert_eq!(drive.trashed_paths(), vec!["a.rnote"]);
    assert_eq!(a.paths(), vec!["a.rnote"]);
}

#[test]
fn library_json_edited_on_both_is_merged() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    rnote_library::Library::open(a.root()).unwrap().create_tag("Parcial", 1).unwrap();
    rnote_library::Library::open(b.root()).unwrap().create_tag("Teoría", 2).unwrap();

    a.sync(&drive);
    let report = b.sync(&drive);
    a.sync(&drive);

    assert!(report.conflicts.is_empty());
    assert_eq!(tag_names(&a), vec!["Parcial", "Teoría"]);
    assert_eq!(tag_names(&b), vec!["Parcial", "Teoría"]);
    assert_eq!(drive.paths(), vec![".rnote-library/library.json"]);
}

#[test]
fn names_differing_only_in_case_do_not_overwrite() {
    let drive = FakeDrive::new();
    drive.add_file("Orga/A.rnote", b"2");
    let a = Device::new("A");
    a.write("Orga/a.rnote", b"1");

    a.sync(&drive);

    assert_eq!(a.read("Orga/a.rnote"), b"1");
    assert_eq!(a.read("Orga/A (2).rnote"), b"2");
    assert_eq!(drive.paths(), vec!["Orga/A (2).rnote", "Orga/a.rnote"]);
}

#[test]
fn conflict_copy_name_is_free() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("a.rnote", b"1");
    b.write("a.rnote", b"2");
    b.write(COPY_B, b"older copy");

    a.sync(&drive);
    let report = b.sync(&drive);

    let second_copy = "a (conflicto, B, 2026-11-10 21.50) (2).rnote";
    assert_eq!(report.conflicts, vec![second_copy]);
    assert_eq!(b.read(second_copy), b"1");
    assert_eq!(b.read(COPY_B), b"older copy");
}
