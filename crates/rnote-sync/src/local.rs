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
/// This library's sync identity, inside [`LIBRARY_DIR`]; not synced.
const LIBRARY_ID_FILE: &str = "sync-id";
/// Where files the sync removes are set aside, inside [`LIBRARY_DIR`]; not synced.
const REMOVED_DIR: &str = ".sync-removed";
/// Characters exFAT, FAT and Windows don't allow in names.
const UNPORTABLE_CHARS: [char; 8] = ['"', '*', ':', '<', '>', '?', '|', '\\'];

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
        _ => is_notebook_name(name) && folders.iter().all(|folder| visible(folder)),
    }
}

/// Whether `name` is one name (not empty, `.` or `..`, no `/`, `\\` or control characters),
/// so joining it to the library folder can't lead outside.
pub(crate) fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.chars().any(|c| c == '/' || c == '\\' || c.is_control())
}

/// `path` with the characters exFAT, FAT and Windows reject replaced by `-`, and the dots
/// and spaces they drop from the end of a folder name removed.
pub(crate) fn portable_path(path: &str) -> String {
    let parts: Vec<String> = path
        .split('/')
        .map(|part| {
            part.chars().map(|c| if UNPORTABLE_CHARS.contains(&c) { '-' } else { c }).collect()
        })
        .collect();
    let last = parts.len().saturating_sub(1);
    parts
        .into_iter()
        .enumerate()
        .map(
            |(index, part)| {
                if index == last { part } else { part.trim_end_matches(['.', ' ']).to_owned() }
            },
        )
        .collect::<Vec<_>>()
        .join("/")
}

fn is_notebook_name(name: &str) -> bool {
    visible(name)
        && is_plain_name(name)
        && name.len() > NOTEBOOK_EXTENSION.len()
        && name.to_lowercase().ends_with(NOTEBOOK_EXTENSION)
}

fn visible(name: &str) -> bool {
    is_plain_name(name) && !name.starts_with('.')
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

/// The SHA-256 of the local file at `path` right now, or `None` if there's none.
pub(crate) fn hash_now(root: &Path, path: &str) -> anyhow::Result<Option<String>> {
    match fs::read(root.join(path)) {
        Ok(bytes) => Ok(Some(sha256_hex(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("Reading {path}")),
    }
}

/// This library's sync identity, created the first time. A state made for another
/// identity doesn't apply here: the folder was changed, replaced or isn't mounted.
pub(crate) fn library_id(root: &Path) -> anyhow::Result<String> {
    let file = root.join(LIBRARY_DIR).join(LIBRARY_ID_FILE);
    match fs::read_to_string(&file) {
        Ok(id) if !id.trim().is_empty() => return Ok(id.trim().to_owned()),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("Reading {}", file.display())),
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    let seed = format!("{nanos}-{}-{}", std::process::id(), root.display());
    let id = sha256_hex(seed.as_bytes())[..32].to_owned();
    write_atomic(root, &format!("{LIBRARY_DIR}/{LIBRARY_ID_FILE}"), id.as_bytes())?;
    Ok(id)
}

/// Moves the local file at `path` out of the library, to a hidden folder that isn't
/// synced, instead of deleting it.
pub(crate) fn set_aside(root: &Path, path: &str, stamp: &str) -> anyhow::Result<()> {
    let dir = root.join(LIBRARY_DIR).join(REMOVED_DIR);
    fs::create_dir_all(&dir).with_context(|| format!("Creating {}", dir.display()))?;
    let flat = path.replace('/', " - ");
    let target = (1..)
        .map(|n| {
            if n == 1 {
                dir.join(format!("{stamp} {flat}"))
            } else {
                dir.join(format!("{stamp} ({n}) {flat}"))
            }
        })
        .find(|candidate| !candidate.exists())
        .context("A free name to set a file aside")?;
    fs::rename(root.join(path), &target).with_context(|| format!("Setting {path} aside"))
}
