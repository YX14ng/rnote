//! Importing Xournal++ notebooks.

use std::fs;

use anyhow::{Context, bail};
use futures::executor::block_on;
use rnote_engine::Engine;
use rnote_engine::engine::EngineSnapshot;
use rnote_engine::engine::import::XoppImportPrefs;

use crate::names::unique_file_name;
use crate::ops::split_path;
use crate::{Library, relative_path, split_extension};

impl Library {
    /// Converts the `.xopp` notebook at `path` to an `.rnote` beside it, with a suffix if
    /// the name is taken, with the tags and cover of the `.xopp`. The `.xopp` stays. Returns the path
    /// of the new notebook.
    pub fn import_xopp(&mut self, path: &str) -> anyhow::Result<String> {
        let file = self.existing_notebook(path)?;
        let bytes = fs::read(&file).with_context(|| format!("Reading {}", file.display()))?;
        let snapshot =
            block_on(EngineSnapshot::load_from_xopp_bytes(bytes, XoppImportPrefs::default()))
                .context("Reading the Xournal++ notebook failed")?;
        let mut engine = Engine::default();
        let _ = engine.load_snapshot(snapshot);
        let (folder, file_name) = split_path(path);
        let (stem, _) = split_extension(file_name);
        let rnote_bytes = block_on(engine.save_as_rnote_bytes(stem.to_owned()))
            .context("The conversion was cancelled")??;
        let dir = self.folder_dir(folder)?;
        let new_path = relative_path(folder, &unique_file_name(&dir, stem, "rnote"));
        crate::write_new_file(&self.root.join(&new_path), &rnote_bytes)?;
        self.meta.copy_notebook(path, &new_path, self.now());
        self.save_meta()?;
        Ok(new_path)
    }

    /// Copies a notebook from outside the library (an `.rnote` or a Xournal++ `.xopp`,
    /// named `file_name`, with `bytes`) into `folder`, with a suffix if the name is taken.
    /// Returns its path.
    pub fn import_notebook(
        &mut self,
        folder: Option<&str>,
        file_name: &str,
        bytes: &[u8],
    ) -> anyhow::Result<String> {
        let (_, extension) = split_extension(file_name);
        if !["rnote", "xopp"].contains(&extension.to_lowercase().as_str()) {
            bail!("{file_name:?} isn't an Rnote or Xournal++ notebook");
        }
        let path = self.free_path(folder, file_name)?;
        crate::write_new_file(&self.root.join(&path), bytes)?;
        Ok(path)
    }

    /// The `.rnote` an import of the `.xopp` at `path` would have made, if it exists: the
    /// one with the same name in the same folder, ignoring case.
    #[must_use]
    pub fn imported_rnote_of(&self, path: &str) -> Option<String> {
        let file = self.notebook_file(path).ok()?;
        let (folder, file_name) = split_path(path);
        let (stem, _) = split_extension(file_name);
        let wanted = format!("{stem}.rnote").to_lowercase();
        let dir = file.parent()?;
        fs::read_dir(dir)
            .ok()?
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .find(|name| name.to_lowercase() == wanted)
            .map(|name| relative_path(folder, &name))
    }
}
