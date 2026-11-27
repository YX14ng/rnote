//! The failures of library operations that a person causes, and can fix, as values the
//! apps word in the person's language. Their `Display` is the English the log gets.

use std::fmt;

/// Why a library operation refused what it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryError {
    /// A name with nothing but spaces.
    EmptyName,
    /// A name starting with a dot, which would hide the file.
    NameStartsWithDot,
    /// A name with a character no file name can have.
    NameHasCharacter(char),
    /// A folder or notebook already has this name.
    NameTaken(String),
    /// A folder that has folders inside, which deleting it would lose.
    FolderHasFolders(String),
    /// A folder that isn't there (any more).
    NoSuchFolder(String),
    /// A notebook that isn't there (any more).
    NoSuchNotebook(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => f.write_str("The name is empty"),
            Self::NameStartsWithDot => f.write_str("The name can't start with a dot"),
            Self::NameHasCharacter(c) => write!(f, "The name can't contain {c:?}"),
            Self::NameTaken(name) => {
                write!(f, "There is already a folder or notebook named {name:?}")
            }
            Self::FolderHasFolders(folder) => {
                write!(f, "{folder:?} has folders inside; move them out first")
            }
            Self::NoSuchFolder(folder) => write!(f, "There is no folder {folder:?}"),
            Self::NoSuchNotebook(path) => write!(f, "There is no notebook {path:?}"),
        }
    }
}

impl std::error::Error for LibraryError {}
