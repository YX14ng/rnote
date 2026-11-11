//! What this device knew of each file after the last sync, kept outside the library.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// A synced file as it was after the last sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Entry {
    /// Drive's id of the file.
    pub id: String,
    /// The MD5 Drive had for it.
    pub md5: String,
    /// When Drive last saw it change.
    pub modified: String,
    /// The SHA-256 of the local file.
    pub sha256: String,
}

/// The sync state of this device.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SyncState {
    /// Drive's id of the `Rnote` folder the entries belong to.
    pub root_id: Option<String>,
    /// The identity of the library folder the entries belong to.
    #[serde(default)]
    pub library_id: Option<String>,
    /// By relative path.
    pub entries: BTreeMap<String, Entry>,
}

impl SyncState {
    /// Reads the state at `path`. A missing one is empty; one that can't be read is set
    /// aside as `<path>.bad` and counts as empty, which only makes the next sync compare
    /// both sides from scratch.
    pub(crate) fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error).with_context(|| format!("Reading {}", path.display())),
        };
        match serde_json::from_slice(&bytes) {
            Ok(state) => Ok(state),
            Err(_) => {
                let bad = path.with_file_name(format!("{}.bad", file_name(path)));
                fs::rename(path, &bad)
                    .with_context(|| format!("Setting aside {}", path.display()))?;
                Ok(Self::default())
            }
        }
    }

    /// Writes the state to `path`, creating its folder, replacing the file at once.
    pub(crate) fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("Creating {}", dir.display()))?;
        }
        let temporary = path.with_file_name(format!("{}.tmp", file_name(path)));
        fs::write(&temporary, serde_json::to_vec(self)?)
            .with_context(|| format!("Writing {}", temporary.display()))?;
        fs::rename(&temporary, path).with_context(|| format!("Replacing {}", path.display()))
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}
