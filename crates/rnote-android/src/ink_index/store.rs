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
    let stored = read(path);
    let mut index = stored
        .clone()
        .filter(|index| index.language == language)
        .unwrap_or_else(|| InkIndex::empty(language));
    let result = change(&mut index);
    // Listing what to read runs after every edit; most times nothing changed.
    if stored.as_ref() != Some(&index) {
        index.save(path)?;
    }
    Ok(result)
}

/// What the recognizer read in one fragment, as the app hands it over.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub(crate) struct Reading {
    /// The fragment's hash.
    pub hash: String,
    /// Its bounds: left, top, right, bottom.
    pub rect: [f64; 4],
    /// The best readings, best first.
    pub candidates: Vec<String>,
}

/// Keeps what the recognizer read in the fragment `hash` (its best `candidates`, up to
/// three) in the index at `path`, if the index is in `language`, the one it was read in.
#[cfg(test)]
pub(crate) fn add_reading(
    path: &Path,
    language: &str,
    hash: &str,
    rect: [f64; 4],
    candidates: &[String],
) -> anyhow::Result<()> {
    let reading = Reading { hash: hash.to_owned(), rect, candidates: candidates.to_vec() };
    add_readings(path, language, &[reading])
}

/// Keeps what the recognizer read in several fragments (their best candidates, up to three)
/// in the index at `path`, if the index is in `language`, the one they were read in. The
/// index is written once: it is rewritten whole, and a long notebook has thousands of
/// fragments.
pub(crate) fn add_readings(
    path: &Path,
    language: &str,
    readings: &[Reading],
) -> anyhow::Result<()> {
    let _guard = INDEX_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut index = match read(path) {
        // Asked for before the language changed: it would empty the index in the new one.
        Some(index) if index.language != language => return Ok(()),
        Some(index) => index,
        None => InkIndex::empty(language),
    };
    for reading in readings {
        let candidates = reading.candidates.iter().take(3).cloned().collect();
        index.entries.insert(reading.hash.clone(), Entry { rect: reading.rect, candidates });
    }
    index.save(path)
}

/// A fragment the recognizer still has to read, and what is written before it on its line,
/// which helps the recognizer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pending {
    pub fragment: Fragment,
    /// The best reading of the fragment before it on its line, if it was read.
    pub before: Option<String>,
    /// The hash of the fragment before it on its line, if any: when that one isn't read
    /// yet, the app reads it first and knows its reading.
    pub previous: Option<String>,
}

/// Drops from `index` the fragments that aren't in `fragments` anymore (which are line by
/// line, as [`super::fragments::fragments`] gives them), and returns the fragments it has
/// no reading of.
pub(crate) fn pending(index: &mut InkIndex, fragments: &[Fragment]) -> Vec<Pending> {
    let live: BTreeSet<&str> = fragments.iter().map(|fragment| fragment.hash.as_str()).collect();
    index.entries.retain(|hash, _| live.contains(hash.as_str()));
    let mut todo = Vec::new();
    for (at, fragment) in fragments.iter().enumerate() {
        if index.entries.contains_key(&fragment.hash) {
            continue;
        }
        let previous = at
            .checked_sub(1)
            .map(|before| &fragments[before])
            .filter(|before| before.line == fragment.line);
        todo.push(Pending {
            fragment: fragment.clone(),
            before: previous
                .and_then(|before| index.entries.get(&before.hash))
                .and_then(|entry| entry.candidates.first().cloned()),
            previous: previous.map(|before| before.hash.clone()),
        });
    }
    todo
}
