//! Writing the notebooks of the library from an editor, on Android or the desktop: a
//! notebook is written whole or not at all, never while a sync writes, and never over a
//! version that changed since the editor read it.

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

use anyhow::{Context, bail};
use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};

use crate::conflict_copy_path;

/// A notebook the editor saved, and where it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenNotebook {
    /// The path written, relative to the library.
    pub path: String,
    /// The SHA-256 of what was written, in lower-case hex.
    pub sha256: String,
    /// Whether the notebook had changed since the editor read it, so the bytes went to a
    /// conflict copy beside it instead.
    pub conflict: bool,
}

/// Writes `bytes`, the notebook the editor has open, to `path` in the library at `root`, if
/// the file there is still the one with hash `expected_sha256` (or missing). If it changed
/// meanwhile, e.g. a sync brought another device's version, the bytes go to a conflict
/// copy beside it and the file stays as it is.
pub fn write_notebook(
    root: &Path,
    path: &str,
    expected_sha256: Option<&str>,
    device: &str,
    now: NaiveDateTime,
    bytes: &[u8],
) -> anyhow::Result<WrittenNotebook> {
    check_notebook_path(path)?;
    let _guard = rnote_library::write_guard();
    let current = match fs::read(root.join(path)) {
        Ok(current) => Some(current),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error).with_context(|| format!("Reading {path}")),
    };
    let changed = match (current, expected_sha256) {
        (None, _) => false,
        (Some(current), Some(expected)) => sha256_hex(&current) != expected,
        (Some(current), None) => !current.is_empty(),
    };
    let target =
        if changed { conflict_copy_path(root, path, device, now) } else { path.to_owned() };
    write_atomic(&root.join(&target), bytes)?;
    Ok(WrittenNotebook { path: target, sha256: sha256_hex(bytes), conflict: changed })
}

/// Creates the notebook `path` in the library at `root` with `bytes`, all at once: it
/// shows up whole or not at all, never replaces a file, and never while a sync writes.
pub fn create_notebook(root: &Path, path: &str, bytes: &[u8]) -> anyhow::Result<()> {
    check_notebook_path(path)?;
    let _guard = rnote_library::write_guard();
    let target = root.join(path);
    let dir = target.parent().context("A notebook has a folder")?;
    let name = target.file_name().context("A notebook has a name")?.to_string_lossy();
    fs::create_dir_all(dir).with_context(|| format!("Creating {}", dir.display()))?;
    // Hidden, so neither the library nor the sync sees it until it is complete.
    let temporary = dir.join(format!(".{name}.create-tmp"));
    // Android keeps apps from making hard links, so the target is checked instead: this
    // process writes the library only under the write guard held here.
    if fs::symlink_metadata(&target).is_ok() {
        bail!("{path} exists already");
    }
    let written = fs::write(&temporary, bytes)
        .with_context(|| format!("Writing {}", temporary.display()))
        .and_then(|()| fs::rename(&temporary, &target).with_context(|| format!("Creating {path}")));
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}

/// Fails unless `path` names a notebook inside the library: relative, without `..`, and
/// outside hidden folders such as `.rnote-library`.
fn check_notebook_path(path: &str) -> anyhow::Result<()> {
    let inside = Path::new(path).components().all(|component| match component {
        Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
        _ => false,
    });
    if path.is_empty() || !inside || !path.ends_with(".rnote") {
        bail!("{path} isn't a notebook of the library");
    }
    Ok(())
}

/// Replaces `target` with `bytes` at once, through a hidden file in the same folder.
fn write_atomic(target: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let dir = target.parent().context("A notebook has a folder")?;
    let name = target.file_name().context("A notebook has a name")?.to_string_lossy();
    // A sync may have removed the folder with the notebook's last copy here.
    fs::create_dir_all(dir).with_context(|| format!("Creating {}", dir.display()))?;
    let temporary = dir.join(format!(".{name}.write-tmp"));
    fs::write(&temporary, bytes).with_context(|| format!("Writing {}", temporary.display()))?;
    fs::rename(&temporary, target).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        anyhow::Error::new(error).context(format!("Replacing {}", target.display()))
    })
}

/// The SHA-256 of `bytes`, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}
