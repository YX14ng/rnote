//! `library.json`: the tags, the tags and cover of each notebook and the trash.
//!
//! Every entry carries the time it last changed, and removals stay as tombstones
//! (`deleted: true`), so copies edited on different devices can be merged entry by entry.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

/// The file name of the metadata, inside [`crate::LIBRARY_DIR`].
pub const META_FILE: &str = "library.json";
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
pub(crate) struct CoverRecord {
    /// The color of the cover, as ARGB.
    pub color: u32,
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
    /// Left out while empty, so a library without covers is written as before them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub notebook_covers: BTreeMap<String, CoverRecord>,
    #[serde(default)]
    pub trash: BTreeMap<String, TrashRecord>,
    /// Keys this version doesn't know, written by a newer one: kept as they are, so saving
    /// never loses them.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            version: VERSION,
            tags: BTreeMap::new(),
            notebook_tags: BTreeMap::new(),
            notebook_covers: BTreeMap::new(),
            trash: BTreeMap::new(),
            extra: serde_json::Map::new(),
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
        let _guard = crate::write_guard();
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

    /// The live cover color of the notebook at `path`.
    pub(crate) fn cover_of(&self, path: &str) -> Option<u32> {
        self.notebook_covers.get(path).filter(|record| !record.deleted).map(|record| record.color)
    }

    /// Sets the cover of the notebook at `path`; `None` removes it, as a tombstone.
    pub(crate) fn set_cover_of(&mut self, path: &str, color: Option<u32>, now: i64) {
        match color {
            Some(color) => {
                self.notebook_covers.insert(
                    path.to_owned(),
                    CoverRecord { color, updated_at: now, deleted: false },
                );
            }
            None => {
                if let Some(record) = self.notebook_covers.get_mut(path).filter(|r| !r.deleted) {
                    record.updated_at = now;
                    record.deleted = true;
                }
            }
        }
    }

    /// Moves what the library keeps of the notebook at `from` (tags and cover) to `to`,
    /// leaving tombstones behind.
    pub(crate) fn move_notebook(&mut self, from: &str, to: &str, now: i64) {
        self.copy_notebook(from, to, now);
        self.forget_notebook(from, now);
    }

    /// Gives the notebook at `to` the tags and cover of the one at `from`.
    pub(crate) fn copy_notebook(&mut self, from: &str, to: &str, now: i64) {
        let tags = self.tags_of(from);
        if !tags.is_empty() {
            self.set_tags_of(to, tags, now);
        }
        if let Some(color) = self.cover_of(from) {
            self.set_cover_of(to, Some(color), now);
        }
    }

    /// Removes the tags and cover of the notebook at `path`, as tombstones.
    pub(crate) fn forget_notebook(&mut self, path: &str, now: i64) {
        if let Some(record) = self.notebook_tags.get_mut(path).filter(|r| !r.deleted) {
            record.tags.clear();
            record.updated_at = now;
            record.deleted = true;
        }
        self.set_cover_of(path, None, now);
    }
}

/// An entry of `library.json` that merges by its change time.
trait Record: Clone + Serialize {
    fn updated_at(&self) -> i64;
    fn deleted(&self) -> bool;
}

macro_rules! impl_record {
    ($($record:ty),*) => {$(
        impl Record for $record {
            fn updated_at(&self) -> i64 {
                self.updated_at
            }

            fn deleted(&self) -> bool {
                self.deleted
            }
        }
    )*};
}

impl_record!(TagRecord, NotebookTagsRecord, CoverRecord, TrashRecord);

/// Merges two copies of `library.json` entry by entry: the newer change of each entry
/// wins, tombstones included. Ties go to the tombstone and then to the larger serialized
/// entry, so every device gets the same result whatever the order. A copy that can't be
/// read, or is of another version, counts as missing; it's an error only if both are.
pub fn merge_meta(local: &[u8], remote: &[u8]) -> anyhow::Result<Vec<u8>> {
    let merged = match (parse(local), parse(remote)) {
        (Some(local), Some(remote)) => Meta {
            version: VERSION,
            tags: merge_map(&local.tags, &remote.tags),
            notebook_tags: merge_map(&local.notebook_tags, &remote.notebook_tags),
            notebook_covers: merge_map(&local.notebook_covers, &remote.notebook_covers),
            trash: merge_map(&local.trash, &remote.trash),
            extra: merge_extra(local.extra, remote.extra),
        },
        (Some(meta), None) | (None, Some(meta)) => meta,
        (None, None) => bail!("Neither copy of {META_FILE} can be read"),
    };
    Ok(serde_json::to_vec_pretty(&merged)?)
}

/// Unknown keys of both copies; the local one wins a key both have.
fn merge_extra(
    local: serde_json::Map<String, serde_json::Value>,
    remote: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut merged = remote;
    merged.extend(local);
    merged
}

fn parse(bytes: &[u8]) -> Option<Meta> {
    serde_json::from_slice::<Meta>(bytes).ok().filter(|meta| meta.version == VERSION)
}

fn merge_map<R: Record>(
    local: &BTreeMap<String, R>,
    remote: &BTreeMap<String, R>,
) -> BTreeMap<String, R> {
    let mut merged = local.clone();
    for (key, theirs) in remote {
        let keep_theirs = match merged.get(key) {
            Some(ours) => wins(theirs, ours),
            None => true,
        };
        if keep_theirs {
            merged.insert(key.clone(), theirs.clone());
        }
    }
    merged
}

/// Whether record `a` wins over record `b`.
fn wins<R: Record>(a: &R, b: &R) -> bool {
    let key = |record: &R| {
        (record.updated_at(), record.deleted(), serde_json::to_string(record).unwrap_or_default())
    };
    key(a) > key(b)
}
