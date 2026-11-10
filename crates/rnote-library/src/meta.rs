//! `library.json`: the tags, the tags of each notebook and the trash.
//!
//! Every entry carries the time it last changed, and removals stay as tombstones
//! (`deleted: true`), so copies edited on different devices can be merged entry by entry.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// The file name, inside the library's data folder.
pub(crate) const META_FILE: &str = "library.json";
/// Where a file that can't be read is set aside.
const BAD_META_FILE: &str = "library.json.bad";
/// The format version this code reads and writes.
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TagRecord {
    pub name: String,
    pub color: u32,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct NotebookTagsRecord {
    pub tags: Vec<String>,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct TrashRecord {
    pub original_path: String,
    pub deleted_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Meta {
    pub version: u32,
    #[serde(default)]
    pub tags: BTreeMap<String, TagRecord>,
    #[serde(default)]
    pub notebook_tags: BTreeMap<String, NotebookTagsRecord>,
    #[serde(default)]
    pub trash: BTreeMap<String, TrashRecord>,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            version: VERSION,
            tags: BTreeMap::new(),
            notebook_tags: BTreeMap::new(),
            trash: BTreeMap::new(),
        }
    }
}

impl Meta {
    /// Reads the metadata in `dir`. A missing file is an empty library; one that can't be
    /// read, or is of another version, is set aside as `library.json.bad` and replaced by
    /// an empty one, so a damaged file never keeps the notebooks from showing.
    pub(crate) fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(META_FILE);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error).with_context(|| format!("Reading {}", path.display())),
        };
        match serde_json::from_slice::<Self>(&bytes) {
            Ok(meta) if meta.version == VERSION => Ok(meta),
            _ => {
                fs::rename(&path, dir.join(BAD_META_FILE))
                    .with_context(|| format!("Setting aside {}", path.display()))?;
                Ok(Self::default())
            }
        }
    }

    /// Writes the metadata to `dir`, replacing the file at once so a crash never leaves
    /// half of it.
    pub(crate) fn save(&self, dir: &Path) -> anyhow::Result<()> {
        let path = dir.join(META_FILE);
        let temporary = dir.join(format!("{META_FILE}.tmp"));
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("Writing {}", temporary.display()))?;
        fs::rename(&temporary, &path).with_context(|| format!("Replacing {}", path.display()))
    }

    /// The live tags of the notebook at `path`.
    pub(crate) fn tags_of(&self, path: &str) -> Vec<String> {
        self.notebook_tags
            .get(path)
            .filter(|record| !record.deleted)
            .map(|record| {
                record
                    .tags
                    .iter()
                    .filter(|id| self.tags.get(*id).is_some_and(|tag| !tag.deleted))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Sets the tags of the notebook at `path`.
    pub(crate) fn set_tags_of(&mut self, path: &str, tags: Vec<String>, now: i64) {
        self.notebook_tags
            .insert(path.to_owned(), NotebookTagsRecord { tags, updated_at: now, deleted: false });
    }

    /// Moves the tags of the notebook at `from` to `to`, leaving a tombstone behind.
    pub(crate) fn move_tags(&mut self, from: &str, to: &str, now: i64) {
        let tags = self.tags_of(from);
        if tags.is_empty() {
            return;
        }
        self.forget_tags(from, now);
        self.set_tags_of(to, tags, now);
    }

    /// Removes the tags of the notebook at `path`, as a tombstone.
    pub(crate) fn forget_tags(&mut self, path: &str, now: i64) {
        if let Some(record) = self.notebook_tags.get_mut(path) {
            record.tags.clear();
            record.updated_at = now;
            record.deleted = true;
        }
    }
}
