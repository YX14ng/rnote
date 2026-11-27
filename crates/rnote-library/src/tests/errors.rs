//! The failures a person can cause come back as [`LibraryError`]s, which the apps word in
//! the person's language.

use super::{library_at, write};
use crate::LibraryError;

fn kind(error: &anyhow::Error) -> Option<&LibraryError> {
    error.chain().find_map(|cause| cause.downcast_ref::<LibraryError>())
}

#[test]
fn bad_names_say_what_is_wrong() {
    let dir = tempfile::tempdir().unwrap();
    let mut library = library_at(dir.path());

    let empty = library.create_folder("  ").unwrap_err();
    let dot = library.create_folder(".hidden").unwrap_err();
    let slash = library.create_folder("a/b").unwrap_err();

    assert_eq!(kind(&empty), Some(&LibraryError::EmptyName));
    assert_eq!(kind(&dot), Some(&LibraryError::NameStartsWithDot));
    assert_eq!(kind(&slash), Some(&LibraryError::NameHasCharacter('/')));
}

#[test]
fn a_taken_name_says_which() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Orga/a.rnote", b"1");
    write(dir.path(), "b.rnote", b"2");
    let mut library = library_at(dir.path());

    let folder = library.create_folder("Orga").unwrap_err();
    let notebook = library.rename("b.rnote", "a").map(|_| ()).err();
    // Now `a.rnote` at the top, whose name `Orga` already has.
    let into_folder = library.move_to("a.rnote", Some("Orga")).map(|_| ()).err();

    assert_eq!(kind(&folder), Some(&LibraryError::NameTaken("Orga".to_owned())));
    assert!(notebook.is_none(), "a name free in its own folder is fine");
    // Moving keeps both: a free name is picked instead of failing.
    assert!(into_folder.is_none());
}

#[test]
fn a_folder_with_folders_inside_is_not_deleted() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Orga/Sub/a.rnote", b"1");
    let mut library = library_at(dir.path());

    let error = library.delete_folder("Orga").unwrap_err();

    assert_eq!(kind(&error), Some(&LibraryError::FolderHasFolders("Orga".to_owned())));
}

#[test]
fn something_gone_says_so() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"1");
    let mut library = library_at(dir.path());

    let notebook = library.rename("missing.rnote", "x").unwrap_err();
    let folder = library.move_to("a.rnote", Some("Nowhere")).unwrap_err();

    assert_eq!(kind(&notebook), Some(&LibraryError::NoSuchNotebook("missing.rnote".to_owned())));
    assert_eq!(kind(&folder), Some(&LibraryError::NoSuchFolder("Nowhere".to_owned())));
}
