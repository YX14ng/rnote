//! The notebook library shared by the Rnote apps: the notebooks in a root folder and its
//! first-level folders, with tags and a trash kept in `.rnote-library/`.
//!
//! The library is plain files, so other programs (and the sync) can work on it too.

pub mod names;
mod scan;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// The folder with the library's own data, inside the root.
pub const LIBRARY_DIR: &str = ".rnote-library";

/// Milliseconds since the Unix epoch, injectable so tests control time.
pub type Clock = fn() -> i64;

/// The current time, in milliseconds since the Unix epoch.
#[must_use]
pub fn system_clock() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
}

/// What a notebook file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotebookKind {
    /// An Rnote document.
    Rnote,
    /// A Xournal++ document, offered for import.
    Xopp,
}

/// A notebook in the library.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Notebook {
    /// Path relative to the root, with `/` as separator, e.g. `Álgebra/Parcial 1.rnote`.
    pub path: String,
    /// The file name without its extension.
    pub name: String,
    /// The folder it is in, or `None` at the root ("uncategorized").
    pub folder: Option<String>,
    /// What the file is.
    pub kind: NotebookKind,
    /// Last modification, in milliseconds since the Unix epoch.
    pub modified_ms: i64,
    /// Size in bytes.
    pub size: u64,
    /// Ids of its tags.
    pub tags: Vec<String>,
}

/// A library rooted at a folder.
#[derive(Debug)]
pub struct Library {
    root: PathBuf,
    #[allow(dead_code)] // Used by the operations that record changes.
    clock: Clock,
}

impl Library {
    /// Opens the library at `root`, creating its data folder if needed.
    pub fn open(root: &Path) -> anyhow::Result<Self> {
        Self::open_with_clock(root, system_clock)
    }

    /// Like [`Library::open`], with `clock` as the source of time.
    pub fn open_with_clock(root: &Path, clock: Clock) -> anyhow::Result<Self> {
        std::fs::create_dir_all(root.join(LIBRARY_DIR))?;
        Ok(Self { root: root.to_owned(), clock })
    }

    /// The root folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The notebooks at the root and in its first-level folders.
    pub fn notebooks(&self) -> anyhow::Result<Vec<Notebook>> {
        scan::notebooks(&self.root)
    }

    /// The first-level folders.
    pub fn folders(&self) -> anyhow::Result<Vec<String>> {
        scan::folders(&self.root)
    }
}
