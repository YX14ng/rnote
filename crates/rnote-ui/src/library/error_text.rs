//! What the user reads when something fails: the library's and the disk's errors in their
//! language, instead of the English chain the log keeps.

use std::io;

use gettextrs::gettext;
use rnote_library::LibraryError;

/// The sentence telling the user why `error` happened.
pub(crate) fn user_message(error: &anyhow::Error) -> String {
    if let Some(error) = error.chain().find_map(|e| e.downcast_ref::<LibraryError>()) {
        return library_message(error);
    }
    if let Some(error) = error.chain().find_map(|e| e.downcast_ref::<io::Error>())
        && let Some(message) = io_message(error.kind())
    {
        return message;
    }
    gettext("Something unexpected went wrong ({error})").replace("{error}", &format!("{error:#}"))
}

fn library_message(error: &LibraryError) -> String {
    let quoted = |text: &str, name: &str| gettext(text).replace("{name}", name);
    match error {
        LibraryError::EmptyName => gettext("The name is empty"),
        LibraryError::NameStartsWithDot => gettext("A name can’t start with a dot"),
        LibraryError::NameHasCharacter(c) => {
            quoted("A name can’t contain “{name}”", &c.to_string())
        }
        LibraryError::NameTaken(name) => {
            quoted("There is already a folder or notebook named “{name}”", name)
        }
        LibraryError::FolderHasFolders(folder) => quoted(
            "“{name}” has folders inside: move them out before deleting it",
            folder,
        ),
        LibraryError::NoSuchFolder(folder) => {
            quoted("The folder “{name}” no longer exists", folder)
        }
        LibraryError::NoSuchNotebook(path) => {
            quoted("The notebook “{name}” no longer exists", path)
        }
    }
}

fn io_message(kind: io::ErrorKind) -> Option<String> {
    Some(match kind {
        io::ErrorKind::StorageFull => gettext("There is no space left on the disk"),
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            gettext("Rnote has no permission to use that file or folder")
        }
        io::ErrorKind::NotFound => gettext("The file or folder no longer exists"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_errors_are_worded_for_people() {
        let taken = anyhow::Error::new(LibraryError::NameTaken("Orga".to_owned()));
        let bad = anyhow::Error::new(LibraryError::NameHasCharacter('/'));

        assert_eq!(
            user_message(&taken),
            "There is already a folder or notebook named “Orga”"
        );
        assert_eq!(user_message(&bad), "A name can’t contain “/”");
    }

    #[test]
    fn a_library_error_under_context_is_still_found() {
        let error = anyhow::Error::new(LibraryError::EmptyName).context("Renaming x");

        assert_eq!(user_message(&error), "The name is empty");
    }

    #[test]
    fn disk_errors_say_what_happened() {
        let full =
            anyhow::Error::new(io::Error::from(io::ErrorKind::StorageFull)).context("Writing a");
        let denied = anyhow::Error::new(io::Error::from(io::ErrorKind::PermissionDenied));

        assert_eq!(user_message(&full), "There is no space left on the disk");
        assert_eq!(
            user_message(&denied),
            "Rnote has no permission to use that file or folder"
        );
    }

    #[test]
    fn anything_else_keeps_its_details() {
        let odd = anyhow::anyhow!("disk exploded");

        assert_eq!(
            user_message(&odd),
            "Something unexpected went wrong (disk exploded)"
        );
    }
}
