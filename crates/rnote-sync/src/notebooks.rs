//! Writing the notebooks of the library from an editor, on Android or the desktop: a
//! notebook is written whole or not at all, never while a sync writes, and never over a
//! version that changed since the editor read it.

use std::fs;
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
    // Moved (renamed, or by a sync) since the editor read it: saved at its new place, not
    // brought back at the old one, which would leave the latest strokes in a stray copy.
    let moved = moved_since_read(root, path, expected_sha256);
    let path = moved.as_deref().unwrap_or(path);
    // Hashed a piece at a time: the notebook isn't held in memory a second time to compare.
    let current =
        crate::local::sha256_file(&root.join(path)).with_context(|| format!("Reading {path}"))?;
    let changed = match (current, expected_sha256) {
        (None, _) => false,
        (Some(current), Some(expected)) => current != expected,
        (Some(_), None) => fs::metadata(root.join(path)).is_ok_and(|meta| meta.len() > 0),
    };
    let target =
        if changed { conflict_copy_path(root, path, device, now) } else { path.to_owned() };
    write_atomic(&root.join(&target), bytes)?;
    Ok(WrittenNotebook { path: target, sha256: sha256_hex(bytes), conflict: changed })
}

/// Where the notebook the editor read at `path` (known by `expected_sha256`) is now, if it
/// isn't there anymore because it was moved: relative to `root`, with `/`.
fn moved_since_read(root: &Path, path: &str, expected_sha256: Option<&str>) -> Option<String> {
    let old = root.join(path);
    if expected_sha256.is_none() || old.exists() {
        return None;
    }
    let new = rnote_library::moved_to(&old).filter(|new| new.exists())?;
    let relative = new.strip_prefix(root).ok()?;
    let relative: Vec<String> =
        relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
    let relative = relative.join("/");
    check_notebook_path(&relative).ok()?;
    Some(relative)
}

/// Creates the notebook `path` in the library at `root` with `bytes`, all at once: it
/// shows up whole or not at all, never replaces a file, and never while a sync writes.
pub fn create_notebook(root: &Path, path: &str, bytes: &[u8]) -> anyhow::Result<()> {
    check_notebook_path(path)?;
    rnote_library::write_new_file(&root.join(path), bytes)
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
    rnote_library::replace_durably(&temporary, target, bytes)
}

/// The SHA-256 of `bytes`, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}
