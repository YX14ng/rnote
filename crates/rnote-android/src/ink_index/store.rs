//! The local index of what the recognizer read in each fragment, one file per notebook in
//! the app's cache. It isn't synced: every device builds its own.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::fragments::Fragment;

/// What the recognizer read in one fragment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Entry {
    /// The fragment's bounds when it was read: left, top, right, bottom.
    pub rect: [f64; 4],
    /// The best readings, best first (up to three).
    pub candidates: Vec<String>,
}

/// The index of one notebook.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct InkIndex {
    /// The language the writing was read in, as a BCP 47 tag.
    pub language: String,
    /// The readings, by the hash of the fragment's strokes.
    pub entries: BTreeMap<String, Entry>,
}

impl InkIndex {
    /// An index with nothing read yet, in `language`.
    pub(crate) fn empty(language: &str) -> Self {
        Self { language: language.to_owned(), entries: BTreeMap::new() }
    }

    /// The index at `path` if it was read in `language`; else an empty one in `language`,
    /// so changing the language reads everything again. A missing or unreadable file is
    /// an empty index too: it is only a cache.
    pub(crate) fn load(path: &Path, language: &str) -> Self {
        read(path)
            .filter(|index| index.language == language)
            .unwrap_or_else(|| Self::empty(language))
    }

    /// The language of the index at `path`, if there is a readable one.
    pub(crate) fn stored_language(path: &Path) -> Option<String> {
        read(path).map(|index| index.language)
    }

    /// Writes the index to `path` at once, through a file beside it.
    pub(crate) fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("Creating {}", dir.display()))?;
        }
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec(self)?)
            .with_context(|| format!("Writing {}", temporary.display()))?;
        fs::rename(&temporary, path).with_context(|| format!("Replacing {}", path.display()))
    }
}

fn read(path: &Path) -> Option<InkIndex> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// Kept while an index file is read, changed and written: the recognizer adds readings on
/// one thread while the document prunes the index on another.
static INDEX_LOCK: Mutex<()> = Mutex::new(());

/// Runs `change` on the index at `path` in `language` and saves it, with no other change
/// to an index in between.
pub(crate) fn with_index<T>(
    path: &Path,
    language: &str,
    change: impl FnOnce(&mut InkIndex) -> T,
) -> anyhow::Result<T> {
    let _guard = INDEX_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut index = InkIndex::load(path, language);
    let result = change(&mut index);
    index.save(path)?;
    Ok(result)
}

/// Keeps what the recognizer read in the fragment `hash` (its best `candidates`, up to
/// three) in the index at `path`, if the index is in `language`, the one it was read in.
pub(crate) fn add_reading(
    path: &Path,
    language: &str,
    hash: &str,
    rect: [f64; 4],
    candidates: &[String],
) -> anyhow::Result<()> {
    let candidates = candidates.iter().take(3).cloned().collect();
    let _guard = INDEX_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut index = match read(path) {
        // Asked for before the language changed: it would empty the index in the new one.
        Some(index) if index.language != language => return Ok(()),
        Some(index) => index,
        None => InkIndex::empty(language),
    };
    index.entries.insert(hash.to_owned(), Entry { rect, candidates });
    index.save(path)
}

/// Drops from `index` the fragments that aren't in `fragments` anymore, and returns the
/// fragments it has no reading of.
pub(crate) fn pending(index: &mut InkIndex, fragments: &[Fragment]) -> Vec<Fragment> {
    let live: BTreeSet<&str> = fragments.iter().map(|fragment| fragment.hash.as_str()).collect();
    index.entries.retain(|hash, _| live.contains(hash.as_str()));
    fragments
        .iter()
        .filter(|fragment| !index.entries.contains_key(&fragment.hash))
        .cloned()
        .collect()
}
