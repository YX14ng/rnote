//! A cache of notebook thumbnails on disk, so a library opens without rendering every
//! notebook again.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, bail};

use crate::thumbnail::path_key;
use crate::{Notebook, NotebookKind, render_thumbnail, thumbnail_key};

/// Thumbnails kept as `<dir>/<key>-<size>.png`, where the key changes whenever the
/// notebook does (see [`thumbnail_key`]).
#[derive(Debug, Clone)]
pub struct ThumbnailCache {
    dir: PathBuf,
}

impl ThumbnailCache {
    /// A cache in `dir`, created when the first thumbnail is written.
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// The PNG thumbnail of `notebook` of the library at `root`, at most `size_px` wide and
    /// high: the cached one, else rendered and cached. Xournal++ notebooks have none.
    pub fn get_or_render(
        &self,
        root: &Path,
        notebook: &Notebook,
        size_px: u32,
    ) -> anyhow::Result<Vec<u8>> {
        if notebook.kind == NotebookKind::Xopp {
            bail!("Xournal++ notebooks have no thumbnail");
        }
        let file = self.dir.join(format!("{}-{size_px}.png", thumbnail_key(notebook)));
        if let Ok(png) = fs::read(&file) {
            return Ok(png);
        }
        let source = root.join(&notebook.path);
        let bytes = fs::read(&source).with_context(|| format!("Reading {}", source.display()))?;
        let png = render_thumbnail(bytes, size_px)?;
        self.write(&file, &png)?;
        self.forget_older(&notebook.path, size_px, &file);
        Ok(png)
    }

    /// Writes `png` to `file` through a temporary file, so a reader never sees half of it.
    /// Deletes the thumbnails of notebooks other than `notebooks`, e.g. of those renamed,
    /// moved or sent to the trash, which would otherwise stay in the cache for good.
    pub fn forget_others(&self, notebooks: &[Notebook]) {
        let live: std::collections::HashSet<String> =
            notebooks.iter().map(|notebook| path_key(&notebook.path)).collect();
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some((key, _)) = name.split_once('-') else {
                continue;
            };
            if name.ends_with(".png") && !live.contains(key) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    fn write(&self, file: &Path, png: &[u8]) -> anyhow::Result<()> {
        // Unique per write: two renders of one notebook at once mustn't share a temporary.
        static WRITES: AtomicU64 = AtomicU64::new(0);
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("Creating {}", self.dir.display()))?;
        let write = WRITES.fetch_add(1, Ordering::Relaxed);
        let tmp = file.with_extension(format!("png.{}.{write}.tmp", std::process::id()));
        fs::write(&tmp, png).with_context(|| format!("Writing {}", tmp.display()))?;
        fs::rename(&tmp, file).with_context(|| format!("Writing {}", file.display()))
    }

    /// Removes the thumbnails of earlier versions of the notebook at `path` at `size_px`,
    /// keeping `current`: each save of a notebook used to leave one more behind.
    fn forget_older(&self, path: &str, size_px: u32, current: &Path) {
        let prefix = format!("{}-", path_key(path));
        let suffix = format!("-{size_px}.png");
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with(&prefix) && name.ends_with(&suffix) && entry.path() != current {
                // Only a cache: a file that stays is just space.
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}
