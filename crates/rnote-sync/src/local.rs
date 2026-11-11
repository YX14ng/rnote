//! The library folder on this device: which files are synced, their hashes, and writing
//! them without leaving half a file behind.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Context;
use rnote_library::{LIBRARY_DIR, META_FILE};
use sha2::{Digest, Sha256};

/// The extension of notebooks.
const NOTEBOOK_EXTENSION: &str = ".rnote";
/// The library's trash folder, inside [`LIBRARY_DIR`].
const TRASH_DIR: &str = "trash";
/// Appended to the temporary copy of a file being written (which is hidden too).
const TEMPORARY_SUFFIX: &str = ".sync-tmp";

/// Whether the file at the relative `path` (with `/`) is synced: notebooks outside hidden
/// folders, `library.json` and the notebooks in the library's trash.
pub(crate) fn is_synced(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let Some((name, folders)) = parts.split_last() else {
        return false;
    };
    match folders {
        [LIBRARY_DIR] => *name == META_FILE,
        [LIBRARY_DIR, TRASH_DIR] => is_notebook_name(name),
        _ => {
            is_notebook_name(name)
                && folders.iter().all(|folder| visible(folder) && !folder.is_empty())
        }
    }
}

fn is_notebook_name(name: &str) -> bool {
    visible(name)
        && name.len() > NOTEBOOK_EXTENSION.len()
        && name.to_lowercase().ends_with(NOTEBOOK_EXTENSION)
}

fn visible(name: &str) -> bool {
    !name.starts_with('.')
}

/// The synced files under `root`: relative path (with `/`) → SHA-256 of the content, in
/// hex. Symlinks aren't followed.
pub(crate) fn scan(root: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    scan_dir(root, "", &mut files)?;
    Ok(files)
}

fn scan_dir(root: &Path, prefix: &str, files: &mut BTreeMap<String, String>) -> anyhow::Result<()> {
    let dir = root.join(prefix);
    let entries = fs::read_dir(&dir).with_context(|| format!("Reading {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let kind = entry.file_type()?;
        if kind.is_dir() && (visible(&name) || path == LIBRARY_DIR) {
            scan_dir(root, &path, files)?;
        } else if kind.is_file() && is_synced(&path) {
            let bytes = fs::read(entry.path()).with_context(|| format!("Reading {path}"))?;
            files.insert(path, sha256_hex(&bytes));
        }
    }
    Ok(())
}

/// The SHA-256 of `bytes`, in lower-case hex.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    crate::hex(&Sha256::digest(bytes))
}

/// Writes `bytes` to the relative `path` under `root`, creating its folders, through a
/// hidden temporary file renamed into place.
pub(crate) fn write_atomic(root: &Path, path: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let target = root.join(path);
    let dir = target.parent().context("A file path has a folder")?;
    fs::create_dir_all(dir).with_context(|| format!("Creating {}", dir.display()))?;
    let name = target.file_name().context("A file path has a name")?.to_string_lossy();
    let temporary = dir.join(format!(".{name}{TEMPORARY_SUFFIX}"));
    fs::write(&temporary, bytes).with_context(|| format!("Writing {}", temporary.display()))?;
    fs::rename(&temporary, &target).with_context(|| format!("Replacing {path}"))
}
