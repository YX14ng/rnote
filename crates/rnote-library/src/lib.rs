//! The notebook library shared by the Rnote apps: the notebooks in a root folder and its
//! first-level folders, with tags and a trash kept in `.rnote-library/`.
//!
//! The library is plain files, so other programs (and the sync) can work on it too.

mod meta;
pub mod names;
mod ops;
mod scan;
mod tags;
mod trash;

#[cfg(test)]
mod tests;

use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, bail};
use serde::Serialize;

use crate::meta::Meta;

pub use crate::tags::Tag;
pub use crate::trash::TrashEntry;

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
    clock: Clock,
    meta: Meta,
}

impl Library {
    /// Opens the library at `root`, creating its data folder if needed.
    pub fn open(root: &Path) -> anyhow::Result<Self> {
        Self::open_with_clock(root, system_clock)
    }

    /// Like [`Library::open`], with `clock` as the source of time.
    pub fn open_with_clock(root: &Path, clock: Clock) -> anyhow::Result<Self> {
        let data_dir = root.join(LIBRARY_DIR);
        std::fs::create_dir_all(data_dir.join(trash::TRASH_DIR))
            .with_context(|| format!("Creating {}", data_dir.display()))?;
        let meta = Meta::load(&data_dir)?;
        Ok(Self { root: root.to_owned(), clock, meta })
    }

    /// The root folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The notebooks at the root and in its first-level folders.
    pub fn notebooks(&self) -> anyhow::Result<Vec<Notebook>> {
        let mut notebooks = scan::notebooks(&self.root)?;
        for notebook in &mut notebooks {
            notebook.tags = self.meta.tags_of(&notebook.path);
        }
        Ok(notebooks)
    }

    /// The first-level folders.
    pub fn folders(&self) -> anyhow::Result<Vec<String>> {
        scan::folders(&self.root)
    }

    fn data_dir(&self) -> PathBuf {
        self.root.join(LIBRARY_DIR)
    }

    fn save_meta(&self) -> anyhow::Result<()> {
        self.meta.save(&self.data_dir())
    }

    fn now(&self) -> i64 {
        (self.clock)()
    }

    /// The file of the notebook at the relative `path`, which must name a file at the root
    /// or in a first-level folder, never outside the library or in its data folder.
    fn notebook_file(&self, path: &str) -> anyhow::Result<PathBuf> {
        let components: Vec<_> = Path::new(path).components().collect();
        let valid = !components.is_empty()
            && components.len() <= 2
            && components.iter().all(|component| match component {
                Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
                _ => false,
            });
        if !valid {
            bail!("{path:?} is not a notebook of the library");
        }
        Ok(self.root.join(path))
    }

    /// The directory of `folder`, or the root for `None`.
    fn folder_dir(&self, folder: Option<&str>) -> anyhow::Result<PathBuf> {
        match folder {
            None => Ok(self.root.clone()),
            Some(folder) if names::validate_name(folder).ok().as_deref() == Some(folder) => {
                Ok(self.root.join(folder))
            }
            Some(folder) => bail!("{folder:?} is not a folder of the library"),
        }
    }
}

/// The relative path of `file_name` in `folder`.
fn relative_path(folder: Option<&str>, file_name: &str) -> String {
    folder.map_or_else(|| file_name.to_owned(), |folder| format!("{folder}/{file_name}"))
}

/// Splits `file_name` into stem and extension (without the dot).
fn split_extension(file_name: &str) -> (&str, &str) {
    file_name.rsplit_once('.').unwrap_or((file_name, ""))
}
