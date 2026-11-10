//! A cache of notebook thumbnails on disk, so a library opens without rendering every
//! notebook again.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

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
        Ok(png)
    }

    /// Writes `png` to `file` through a temporary file, so a reader never sees half of it.
    fn write(&self, file: &Path, png: &[u8]) -> anyhow::Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("Creating {}", self.dir.display()))?;
        let tmp = file.with_extension(format!("png.{}.tmp", std::process::id()));
        fs::write(&tmp, png).with_context(|| format!("Writing {}", tmp.display()))?;
        fs::rename(&tmp, file).with_context(|| format!("Writing {}", file.display()))
    }
}
