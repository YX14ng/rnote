//! Covers: a color for a notebook, to tell subjects apart at a glance.

use crate::Library;

impl Library {
    /// Sets the cover color (ARGB) of the notebook at `path`; `None` removes it, so the
    /// notebook shows its first page again.
    pub fn set_cover(&mut self, path: &str, color: Option<u32>) -> anyhow::Result<()> {
        self.existing_notebook(path)?;
        self.meta.set_cover_of(path, color, self.now());
        self.save_meta()
    }
}
