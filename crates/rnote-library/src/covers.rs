//! Covers: a color for a notebook, to tell subjects apart at a glance.

use crate::Library;

/// The colors (ARGB) a cover can have, the same in every app: blue, green, orange, red,
/// violet, turquoise, mustard and graphite.
pub const COVER_COLORS: [u32; 8] = [
    0xFF3D_6FB6,
    0xFF3F_8F6B,
    0xFFC9_773A,
    0xFFB5_524E,
    0xFF7A_5BB0,
    0xFF2F_8C93,
    0xFFB8_922E,
    0xFF5E_6470,
];

impl Library {
    /// Sets the cover color (ARGB) of the notebook at `path`; `None` removes it, so the
    /// notebook shows its first page again.
    pub fn set_cover(&mut self, path: &str, color: Option<u32>) -> anyhow::Result<()> {
        self.existing_notebook(path)?;
        self.meta.set_cover_of(path, color, self.now());
        self.save_meta()
    }
}
