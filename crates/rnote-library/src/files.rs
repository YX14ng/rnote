//! Writing a new file of the library all at once.

use std::fs;
use std::path::Path;

use anyhow::{Context, bail};

/// Writes `bytes` as the new file `target`, all at once: it shows up whole or not at all,
/// never replaces a file, and never while another write of this process (the sync, the
/// library's metadata) is checking or changing files. Creates its folder if needed.
pub fn write_new_file(target: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let _guard = crate::write_guard();
    let dir = target.parent().context("A file has a folder")?;
    let name = target.file_name().context("A file has a name")?.to_string_lossy();
    fs::create_dir_all(dir).with_context(|| format!("Creating {}", dir.display()))?;
    // Hidden, so neither the library nor the sync sees it until it is complete.
    let temporary = dir.join(format!(".{name}.create-tmp"));
    // Android keeps apps from making hard links, so the target is checked instead: this
    // process writes the library only under the write guard held here.
    if fs::symlink_metadata(target).is_ok() {
        bail!("{} exists already", target.display());
    }
    let written = fs::write(&temporary, bytes)
        .with_context(|| format!("Writing {}", temporary.display()))
        .and_then(|()| {
            fs::rename(&temporary, target).with_context(|| format!("Creating {}", target.display()))
        });
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}
