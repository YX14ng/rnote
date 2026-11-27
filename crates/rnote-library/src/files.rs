//! Writing files of the library all at once, so that a crash or a power cut leaves either
//! the old file or the new one, never a torn or empty one.

use std::fs::{self, File};
use std::io::Write;
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
    let written = write_synced(&temporary, bytes).and_then(|()| {
        fs::rename(&temporary, target).with_context(|| format!("Creating {}", target.display()))
    });
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written?;
    sync_folder(dir);
    Ok(())
}

/// Replaces `target` with `bytes` through `temporary`, a hidden file in the same folder: the
/// bytes reach the disk before the rename, and the rename before this returns. The temporary
/// file is removed when anything fails, so a full disk doesn't keep its space.
pub fn replace_durably(temporary: &Path, target: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let replaced = write_synced(temporary, bytes).and_then(|()| {
        fs::rename(temporary, target).with_context(|| format!("Replacing {}", target.display()))
    });
    if replaced.is_err() {
        let _ = fs::remove_file(temporary);
    }
    replaced?;
    if let Some(dir) = target.parent() {
        sync_folder(dir);
    }
    Ok(())
}

/// Moves the damaged file at `path` out of the way, to `<name>.bad`, or `<name>.bad.2`, ... when
/// earlier damaged copies are there already: each may hold what only it still has.
pub fn set_aside(path: &Path) -> anyhow::Result<()> {
    let name = path.file_name().context("A file has a name")?.to_string_lossy().into_owned();
    let free = (1..)
        .map(|n| match n {
            1 => path.with_file_name(format!("{name}.bad")),
            n => path.with_file_name(format!("{name}.bad.{n}")),
        })
        .find(|candidate| fs::symlink_metadata(candidate).is_err())
        .context("A free name to set a file aside")?;
    fs::rename(path, &free).with_context(|| format!("Setting aside {}", path.display()))
}

/// Writes `bytes` to `path` and waits until they are on the disk.
fn write_synced(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut file = File::create(path).with_context(|| format!("Writing {}", path.display()))?;
    file.write_all(bytes).with_context(|| format!("Writing {}", path.display()))?;
    file.sync_all().with_context(|| format!("Writing {}", path.display()))
}

/// Makes a rename in `dir` reach the disk. Some systems and file systems can't open a
/// folder for that; the rename is still done, only less certain to survive a crash.
fn sync_folder(dir: &Path) {
    let _ = File::open(dir).and_then(|folder| folder.sync_all());
}
