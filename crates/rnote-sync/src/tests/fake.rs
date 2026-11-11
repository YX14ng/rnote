use crate::fake::FakeDrive;
use crate::{Remote, RemoteError};

/// The id of a new `Rnote` folder at the top of `drive`.
fn root(drive: &mut FakeDrive) -> String {
    drive.create_folder(None, crate::ROOT_FOLDER_NAME).unwrap()
}

#[test]
fn create_and_list_builds_paths_under_the_root() {
    let mut drive = FakeDrive::new();
    let root = root(&mut drive);
    let folder = drive.create_folder(Some(&root), "Orga").unwrap();

    drive.create_file(&root, "a.rnote", b"a").unwrap();
    drive.create_file(&folder, "b.rnote", b"b").unwrap();

    assert_eq!(drive.paths(), vec!["Orga/b.rnote", "a.rnote"]);
    assert_eq!(drive.find_root().unwrap(), Some(root));
    let listing = drive.list().unwrap();
    assert_eq!(listing.files.len(), 2);
    assert_eq!(listing.folders.len(), 2);
    assert_eq!(drive.content("Orga/b.rnote").unwrap(), b"b");
}

#[test]
fn same_name_twice_keeps_both_files() {
    let mut drive = FakeDrive::new();
    let root = root(&mut drive);

    let first = drive.create_file(&root, "a.rnote", b"1").unwrap();
    let second = drive.create_file(&root, "a.rnote", b"2").unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(drive.paths(), vec!["a.rnote", "a.rnote"]);
}

#[test]
fn update_and_move_change_content_and_place() {
    let mut drive = FakeDrive::new();
    let root = root(&mut drive);
    let folder = drive.create_folder(Some(&root), "F").unwrap();
    let file = drive.create_file(&root, "a.rnote", b"1").unwrap();

    let updated = drive.update_file(&file.id, b"2").unwrap();
    let moved = drive.move_file(&file.id, &root, &folder, "b.rnote").unwrap();

    assert_ne!(updated.md5, file.md5);
    assert_eq!((moved.parent.as_str(), moved.name.as_str()), (folder.as_str(), "b.rnote"));
    assert_eq!(drive.download(&file.id).unwrap(), b"2");
    assert_eq!(drive.paths(), vec!["F/b.rnote"]);
}

#[test]
fn trash_hides_a_file_from_the_listing_but_keeps_it() {
    let mut drive = FakeDrive::new();
    let root = root(&mut drive);
    let file = drive.create_file(&root, "a.rnote", b"a").unwrap();

    drive.trash(&file.id).unwrap();

    assert!(drive.paths().is_empty());
    assert!(drive.list().unwrap().files.is_empty());
    assert_eq!(drive.trashed_paths(), vec!["a.rnote"]);
}

#[test]
fn trashing_a_folder_hides_its_files() {
    let mut drive = FakeDrive::new();
    let root = root(&mut drive);
    let folder = drive.create_folder(Some(&root), "F").unwrap();
    drive.create_file(&folder, "a.rnote", b"a").unwrap();

    drive.trash(&folder).unwrap();

    assert!(drive.list().unwrap().files.is_empty());
    assert!(drive.paths().is_empty());
}

#[test]
fn trash_root_hides_everything_and_find_root_finds_nothing() {
    let mut drive = FakeDrive::new();
    drive.add_file("F/a.rnote", b"a");

    drive.trash_root();

    assert!(drive.paths().is_empty());
    assert_eq!(drive.find_root().unwrap(), None);
}

#[test]
fn fail_after_makes_later_operations_offline() {
    let mut drive = FakeDrive::new();
    let root = root(&mut drive);
    drive.fail_after(drive.ops() + 1);

    assert!(drive.list().is_ok());
    assert!(matches!(drive.create_file(&root, "a.rnote", b"a"), Err(RemoteError::Offline(_))));
    drive.fail_never();
    assert!(drive.create_file(&root, "a.rnote", b"a").is_ok());
}

#[test]
fn add_file_works_like_another_client_and_is_not_counted() {
    let drive = FakeDrive::new();

    drive.add_file("F/a.rnote", b"a");

    assert_eq!(drive.ops(), 0);
    assert_eq!(drive.paths(), vec!["F/a.rnote"]);
}

#[test]
fn clones_share_the_same_drive() {
    let drive = FakeDrive::new();
    let other = drive.clone();

    other.add_file("a.rnote", b"a");

    assert_eq!(drive.paths(), vec!["a.rnote"]);
}

#[test]
fn md5_is_the_md5_of_the_content() {
    let mut drive = FakeDrive::new();
    let root = root(&mut drive);

    let file = drive.create_file(&root, "a.rnote", b"abc").unwrap();

    assert_eq!(file.md5, "900150983cd24fb0d6963f7d28e17f72");
}

#[test]
fn lose_reply_after_does_the_operation_but_fails_it() {
    let mut drive = FakeDrive::new();
    let root = root(&mut drive);
    drive.lose_reply_after(drive.ops());

    assert!(matches!(drive.create_file(&root, "a.rnote", b"a"), Err(RemoteError::Offline(_))));
    assert!(matches!(drive.list(), Err(RemoteError::Offline(_))));
    assert_eq!(drive.paths(), vec!["a.rnote"]);
}
