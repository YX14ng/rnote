//! The library folder on this device: which files are synced, their hashes, and writing
//! them without leaving half a file behind.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Context;
use rnote_library::{LIBRARY_DIR, META_FILE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The extension of notebooks.
const NOTEBOOK_EXTENSION: &str = ".rnote";
/// The library's trash folder, inside [`LIBRARY_DIR`].
const TRASH_DIR: &str = "trash";
/// Appended to the temporary copy of a file being written (which is hidden too).
const TEMPORARY_SUFFIX: &str = ".sync-tmp";
/// This library's sync identity, inside [`LIBRARY_DIR`]; not synced.
const LIBRARY_ID_FILE: &str = "sync-id";
/// The hashes of the synced files, to tell which changed without reading them all, inside
/// [`LIBRARY_DIR`]; not synced.
const HASH_CACHE_FILE: &str = "sync-hashes.json";
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
    Ok(scan_counting(root)?.files)
}

/// What [`scan_counting`] found.
pub(crate) struct Scan {
    /// Relative path → SHA-256 of the content, in hex.
    pub files: BTreeMap<String, String>,
    /// How many files had to be read: the others hadn't changed since they were hashed.
    /// Only the tests look at it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub hashed: usize,
}

/// Like [`scan`]. Reading every notebook on every sync is what made it slow, so the hashes
/// are kept in [`HASH_CACHE_FILE`] with each file's size, times and inode, and a file is
/// read again only when one of those changed. Saving a notebook replaces the file, which
/// changes them all.
pub(crate) fn scan_counting(root: &Path) -> anyhow::Result<Scan> {
    let cache_path = root.join(LIBRARY_DIR).join(HASH_CACHE_FILE);
    let known: BTreeMap<String, CachedHash> = fs::read(&cache_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut found = BTreeMap::new();
    let mut hashed = 0;
    scan_dir(root, "", &known, &mut found, &mut hashed)?;
    let files = found.iter().map(|(path, cached)| (path.clone(), cached.sha256.clone())).collect();
    if found != known && root.join(LIBRARY_DIR).is_dir() {
        // Only a cache: if it can't be written, the next scan reads the files again.
        if let Ok(bytes) = serde_json::to_vec(&found) {
            let _ = write_atomic(root, &format!("{LIBRARY_DIR}/{HASH_CACHE_FILE}"), &bytes);
        }
    }
    Ok(Scan { files, hashed })
}

/// A file's hash, with what tells whether the file changed since.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CachedHash {
    len: u64,
    modified_ns: u128,
    changed_ns: i128,
    inode: u64,
    sha256: String,
}

impl CachedHash {
    fn stamp(metadata: &fs::Metadata) -> (u64, u128, i128, u64) {
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_nanos());
        #[cfg(unix)]
        let (changed_ns, inode) = {
            use std::os::unix::fs::MetadataExt;
            (
                i128::from(metadata.ctime()) * 1_000_000_000 + i128::from(metadata.ctime_nsec()),
                metadata.ino(),
            )
        };
        #[cfg(not(unix))]
        let (changed_ns, inode) = (0, 0);
        (metadata.len(), modified_ns, changed_ns, inode)
    }

    fn matches(&self, stamp: (u64, u128, i128, u64)) -> bool {
        (self.len, self.modified_ns, self.changed_ns, self.inode) == stamp
    }
}

fn scan_dir(
    root: &Path,
    prefix: &str,
    known: &BTreeMap<String, CachedHash>,
    found: &mut BTreeMap<String, CachedHash>,
    hashed: &mut usize,
) -> anyhow::Result<()> {
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
            scan_dir(root, &path, known, found, hashed)?;
        } else if kind.is_file() && is_synced(&path) {
            let stamp = CachedHash::stamp(&entry.metadata()?);
            if let Some(cached) = known.get(&path).filter(|cached| cached.matches(stamp)) {
                found.insert(path, cached.clone());
                continue;
            }
            let bytes = fs::read(entry.path()).with_context(|| format!("Reading {path}"))?;
            *hashed += 1;
            let sha256 = sha256_hex(&bytes);
            // Changed while it was read: keep the hash, but don't trust it next time.
            let settled =
                fs::metadata(entry.path()).map(|now| CachedHash::stamp(&now)).ok() == Some(stamp);
            let (len, modified_ns, changed_ns, inode) =
                if settled { stamp } else { (u64::MAX, 0, 0, 0) };
            found.insert(path, CachedHash { len, modified_ns, changed_ns, inode, sha256 });
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
