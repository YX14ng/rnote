//! The trash: notebooks sent there stay as files in the library's data folder until they
//! are restored or deleted for good.

use std::fs;
use std::io::ErrorKind;

use anyhow::{Context, bail};
use serde::Serialize;

use crate::meta::TrashRecord;
use crate::names::unique_file_name;
use crate::ops::split_path;
use crate::{LIBRARY_DIR, Library, split_extension};

/// The trash folder, inside the library's data folder.
pub(crate) const TRASH_DIR: &str = "trash";

/// A notebook in the trash.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrashEntry {
    /// Its file name in the trash folder, which identifies it.
    pub id: String,
    /// The name it had, without extension.
    pub name: String,
    /// Where it was, relative to the root.
    pub original_path: String,
    /// When it was sent to the trash, in milliseconds since the Unix epoch.
    pub deleted_at_ms: i64,
}

impl Library {
    /// Sends the notebook at `path` to the trash.
    pub fn trash(&mut self, path: &str) -> anyhow::Result<()> {
        self.existing_notebook(path)?;
        self.trash_file(path)?;
        self.save_meta()
    }

    /// The notebooks in the trash, sorted by id.
    #[must_use]
    pub fn trash_entries(&self) -> Vec<TrashEntry> {
        self.meta
            .trash
            .iter()
            .filter(|(_, record)| !record.deleted)
            .map(|(id, record)| {
                let (_, file_name) = split_path(&record.original_path);
                TrashEntry {
                    id: id.clone(),
                    name: split_extension(file_name).0.to_owned(),
                    original_path: record.original_path.clone(),
                    deleted_at_ms: record.deleted_at,
                }
            })
            .collect()
    }

    /// Puts a notebook back where it was, recreating its folder if needed and adding a
    /// suffix if its name was taken meanwhile. Returns its path.
    pub fn restore(&mut self, id: &str) -> anyhow::Result<String> {
        let record = self.live_trash_record(id)?.clone();
        let (folder, file_name) = split_path(&record.original_path);
        let dir = self.folder_dir(folder)?;
        fs::create_dir_all(&dir).with_context(|| format!("Creating {}", dir.display()))?;
        let (stem, extension) = split_extension(file_name);
        let free_name = unique_file_name(&dir, stem, extension);
        let restored = crate::relative_path(folder, &free_name);
        let from = self.trash_dir().join(id);
        fs::rename(&from, self.root.join(&restored))
            .with_context(|| format!("Restoring {}", from.display()))?;
        let now = self.now();
        self.meta.move_tags(&trash_key(id), &restored, now);
        self.bury_trash_record(id, now);
        self.save_meta()?;
        Ok(restored)
    }

    /// Deletes a notebook in the trash for good.
    pub fn delete_forever(&mut self, id: &str) -> anyhow::Result<()> {
        self.live_trash_record(id)?;
        self.remove_from_trash(id)?;
        self.save_meta()
    }

    /// Deletes everything in the trash for good.
    pub fn empty_trash(&mut self) -> anyhow::Result<()> {
        for entry in self.trash_entries() {
            self.remove_from_trash(&entry.id)?;
        }
        self.save_meta()
    }

    /// Moves the file at the relative `path` to the trash and records it, without saving
    /// the metadata.
    pub(crate) fn trash_file(&mut self, path: &str) -> anyhow::Result<()> {
        let now = self.now();
        let (_, file_name) = split_path(path);
        let (stem, extension) = split_extension(file_name);
        let id = unique_file_name(&self.trash_dir(), &format!("{now}-{stem}"), extension);
        let from = self.notebook_file(path)?;
        fs::rename(&from, self.trash_dir().join(&id))
            .with_context(|| format!("Moving {} to the trash", from.display()))?;
        // Its tags wait in the trash with it, so a new notebook at its path starts untagged.
        self.meta.move_tags(path, &trash_key(&id), now);
        self.meta.trash.insert(
            id,
            TrashRecord {
                original_path: path.to_owned(),
                deleted_at: now,
                updated_at: now,
                deleted: false,
            },
        );
        Ok(())
    }

    fn remove_from_trash(&mut self, id: &str) -> anyhow::Result<()> {
        let file = self.trash_dir().join(id);
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("Deleting {}", file.display()));
            }
        }
        let now = self.now();
        self.meta.forget_tags(&trash_key(id), now);
        self.bury_trash_record(id, now);
        Ok(())
    }

    fn live_trash_record(&self, id: &str) -> anyhow::Result<&TrashRecord> {
        match self.meta.trash.get(id) {
            Some(record) if !record.deleted => Ok(record),
            _ => bail!("There is nothing in the trash with id {id:?}"),
        }
    }

    fn bury_trash_record(&mut self, id: &str, now: i64) {
        if let Some(record) = self.meta.trash.get_mut(id) {
            record.deleted = true;
            record.updated_at = now;
        }
    }

    fn trash_dir(&self) -> std::path::PathBuf {
        self.root.join(LIBRARY_DIR).join(TRASH_DIR)
    }
}

/// The key under which the tags of the notebook in the trash with `id` wait.
fn trash_key(id: &str) -> String {
    format!("{LIBRARY_DIR}/{TRASH_DIR}/{id}")
}
