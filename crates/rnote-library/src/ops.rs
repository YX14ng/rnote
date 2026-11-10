//! Folders and notebooks: create, rename, move, duplicate.

use std::fs;

use anyhow::{Context, bail};

use crate::names::{name_taken, unique_file_name, validate_name};
use crate::{Library, relative_path, split_extension};

impl Library {
    /// Creates a first-level folder, returning its (trimmed) name.
    pub fn create_folder(&mut self, name: &str) -> anyhow::Result<String> {
        let name = validate_name(name)?;
        let dir = self.root.join(&name);
        if dir.exists() {
            bail!("There is already a folder or notebook named {name:?}");
        }
        fs::create_dir(&dir).with_context(|| format!("Creating {}", dir.display()))?;
        Ok(name)
    }

    /// Renames a folder, keeping the tags of its notebooks. Returns the new name.
    pub fn rename_folder(&mut self, folder: &str, new_name: &str) -> anyhow::Result<String> {
        let from = self.folder_dir(Some(folder))?;
        let new_name = validate_name(new_name)?;
        let to = self.root.join(&new_name);
        if to.exists() {
            bail!("There is already a folder or notebook named {new_name:?}");
        }
        let notebooks: Vec<_> = self
            .notebooks()?
            .into_iter()
            .filter(|notebook| notebook.folder.as_deref() == Some(folder))
            .collect();
        fs::rename(&from, &to).with_context(|| format!("Renaming {}", from.display()))?;
        let now = self.now();
        for notebook in notebooks {
            let file_name = notebook.path.rsplit('/').next().unwrap_or(&notebook.path);
            self.meta.move_tags(&notebook.path, &relative_path(Some(&new_name), file_name), now);
        }
        self.save_meta()?;
        Ok(new_name)
    }

    /// Deletes a folder, sending everything in it to the trash.
    pub fn delete_folder(&mut self, folder: &str) -> anyhow::Result<()> {
        let dir = self.folder_dir(Some(folder))?;
        // Everything is checked before anything moves.
        let mut files = Vec::new();
        for entry in fs::read_dir(&dir).with_context(|| format!("Reading {}", dir.display()))? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                bail!("{folder:?} has folders inside; move them out first");
            }
            let Ok(file_name) = entry.file_name().into_string() else {
                bail!("{folder:?} has a file whose name isn't valid text");
            };
            files.push(file_name);
        }
        for file_name in files {
            // Hidden files too, e.g. other programs' autosaves; recorded one by one, so a
            // failure halfway leaves nothing in the trash without its record.
            self.trash_relative(&relative_path(Some(folder), &file_name))?;
            self.save_meta()?;
        }
        fs::remove_dir(&dir).with_context(|| format!("Removing {}", dir.display()))
    }

    /// Renames the notebook at `path` to `new_name` (without extension) in the same folder.
    /// Returns its new path.
    pub fn rename(&mut self, path: &str, new_name: &str) -> anyhow::Result<String> {
        let from = self.existing_notebook(path)?;
        let new_name = validate_name(new_name)?;
        let (folder, file_name) = split_path(path);
        let (_, extension) = split_extension(file_name);
        let new_file_name = format!("{new_name}.{extension}");
        let new_path = relative_path(folder, &new_file_name);
        if new_path == path {
            return Ok(new_path);
        }
        let to = self.root.join(&new_path);
        let dir = self.folder_dir(folder)?;
        if name_taken(&dir, &new_file_name, Some(&from)) {
            bail!("There is already a notebook named {new_name:?}");
        }
        if to.exists() {
            // On case-insensitive file systems (exFAT, NTFS) the new name is the old file:
            // go through a temporary name.
            let temporary = from.with_extension(format!("{extension}.renaming"));
            fs::rename(&from, &temporary)
                .with_context(|| format!("Renaming {}", from.display()))?;
            fs::rename(&temporary, &to).with_context(|| format!("Renaming {}", from.display()))?;
        } else {
            fs::rename(&from, &to).with_context(|| format!("Renaming {}", from.display()))?;
        }
        self.meta.move_tags(path, &new_path, self.now());
        self.save_meta()?;
        Ok(new_path)
    }

    /// Moves the notebook at `path` to `folder` (`None` for the root), with a suffix if the
    /// name is taken there. Returns its new path.
    pub fn move_to(&mut self, path: &str, folder: Option<&str>) -> anyhow::Result<String> {
        let from = self.existing_notebook(path)?;
        let dir = self.folder_dir(folder)?;
        if !dir.is_dir() {
            bail!("There is no folder {:?}", folder.unwrap_or_default());
        }
        let (current_folder, file_name) = split_path(path);
        if current_folder == folder {
            return Ok(path.to_owned());
        }
        let (stem, extension) = split_extension(file_name);
        let new_path = relative_path(folder, &unique_file_name(&dir, stem, extension));
        fs::rename(&from, self.root.join(&new_path))
            .with_context(|| format!("Moving {}", from.display()))?;
        self.meta.move_tags(path, &new_path, self.now());
        self.save_meta()?;
        Ok(new_path)
    }

    /// Copies the notebook at `path` beside it, with its tags. Returns the copy's path.
    pub fn duplicate(&mut self, path: &str) -> anyhow::Result<String> {
        let from = self.existing_notebook(path)?;
        let (folder, file_name) = split_path(path);
        let (stem, extension) = split_extension(file_name);
        let dir = self.folder_dir(folder)?;
        let copy = relative_path(folder, &unique_file_name(&dir, stem, extension));
        fs::copy(&from, self.root.join(&copy))
            .with_context(|| format!("Copying {}", from.display()))?;
        let tags = self.meta.tags_of(path);
        if !tags.is_empty() {
            self.meta.set_tags_of(&copy, tags, self.now());
            self.save_meta()?;
        }
        Ok(copy)
    }

    /// A free path for a new `.rnote` notebook named `stem` in `folder`. The file isn't
    /// created.
    pub fn new_notebook_path(&self, folder: Option<&str>, stem: &str) -> anyhow::Result<String> {
        let stem = validate_name(stem)?;
        let dir = self.folder_dir(folder)?;
        Ok(relative_path(folder, &unique_file_name(&dir, &stem, "rnote")))
    }

    /// A free path for a file named like `file_name` (a name with extension) in `folder`,
    /// with a suffix if the name is taken, e.g. to copy a file into the library.
    pub fn free_path(&self, folder: Option<&str>, file_name: &str) -> anyhow::Result<String> {
        let (stem, extension) = split_extension(file_name);
        let stem = validate_name(stem)?;
        if extension.is_empty() || extension.contains(['/', '\\']) {
            bail!("{file_name:?} has no valid extension");
        }
        let dir = self.folder_dir(folder)?;
        Ok(relative_path(folder, &unique_file_name(&dir, &stem, extension)))
    }

    /// The file of the notebook at `path`, which must exist.
    pub(crate) fn existing_notebook(&self, path: &str) -> anyhow::Result<std::path::PathBuf> {
        let file = self.notebook_file(path)?;
        if !file.is_file() {
            bail!("There is no notebook {path:?}");
        }
        Ok(file)
    }
}

/// Splits a relative path into its folder (if any) and file name.
pub(crate) fn split_path(path: &str) -> (Option<&str>, &str) {
    match path.split_once('/') {
        Some((folder, file_name)) => (Some(folder), file_name),
        None => (None, path),
    }
}
