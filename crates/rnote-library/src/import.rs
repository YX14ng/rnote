//! Importing Xournal++ notebooks.

use std::fs;

use anyhow::Context;
use futures::executor::block_on;
use rnote_engine::Engine;
use rnote_engine::engine::EngineSnapshot;
use rnote_engine::engine::import::XoppImportPrefs;

use crate::names::unique_file_name;
use crate::ops::split_path;
use crate::{Library, relative_path, split_extension};

impl Library {
    /// Converts the `.xopp` notebook at `path` to an `.rnote` beside it, with a suffix if
    /// the name is taken. The `.xopp` stays. Returns the path of the new notebook.
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
        fs::write(self.root.join(&new_path), rnote_bytes)
            .with_context(|| format!("Writing {new_path}"))?;
        Ok(new_path)
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
