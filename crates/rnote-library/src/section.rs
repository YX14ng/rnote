//! The sections of the library (all, uncategorized, a folder, a tag, the trash) and the
//! order of their notebooks, the same in every app.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::Notebook;

/// A section of the library, as picked in its sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Section {
    /// Every notebook.
    All,
    /// The notebooks at the root, in no folder.
    Uncategorized,
    /// The notebooks in the first-level folder with this name.
    Folder(String),
    /// The notebooks with the tag with this id.
    Tag(String),
    /// The trash, listed with [`crate::Library::trash_entries`].
    Trash,
}

/// How the notebooks of a section are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// The most recently modified first.
    #[default]
    Modified,
    /// By name, ignoring case; the most recently modified first among equal names.
    Name,
}

/// The number of notebooks in each section.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionCounts {
    /// Every notebook.
    pub all: usize,
    /// The notebooks in no folder.
    pub uncategorized: usize,
    /// By folder name; every folder is present, empty ones with 0.
    pub folders: BTreeMap<String, usize>,
    /// By tag id; only tags some notebook has.
    pub tags: BTreeMap<String, usize>,
    /// The entries in the trash.
    pub trash: usize,
}

/// The notebooks of `notebooks` in `section`, in the same order. Empty for the trash.
#[must_use]
pub fn filter(notebooks: &[Notebook], section: &Section) -> Vec<Notebook> {
    notebooks.iter().filter(|notebook| in_section(notebook, section)).cloned().collect()
}

fn in_section(notebook: &Notebook, section: &Section) -> bool {
    match section {
        Section::All => true,
        Section::Uncategorized => notebook.folder.is_none(),
        Section::Folder(folder) => notebook.folder.as_deref() == Some(folder.as_str()),
        Section::Tag(tag) => notebook.tags.contains(tag),
        Section::Trash => false,
    }
}

/// Orders `notebooks` by `order`.
pub fn sort(notebooks: &mut [Notebook], order: SortOrder) {
    let newest_first = |a: &Notebook, b: &Notebook| b.modified_ms.cmp(&a.modified_ms);
    match order {
        SortOrder::Modified => notebooks.sort_by(newest_first),
        SortOrder::Name => notebooks.sort_by(|a, b| match compare_names(&a.name, &b.name) {
            Ordering::Equal => newest_first(a, b),
            ordering => ordering,
        }),
    }
}

/// Compares names ignoring case, char by char like Java's `CASE_INSENSITIVE_ORDER`, so the
/// apps agree on the order.
fn compare_names(a: &str, b: &str) -> Ordering {
    let fold = |c: char| c.to_lowercase();
    a.chars().flat_map(fold).cmp(b.chars().flat_map(fold))
}

/// The counts of every section, given the library's `notebooks`, its `folders` and the
/// number of `trash` entries.
#[must_use]
pub fn counts(notebooks: &[Notebook], folders: &[String], trash: usize) -> SectionCounts {
    let mut counts = SectionCounts {
        all: notebooks.len(),
        folders: folders.iter().map(|folder| (folder.clone(), 0)).collect(),
        trash,
        ..SectionCounts::default()
    };
    for notebook in notebooks {
        match &notebook.folder {
            None => counts.uncategorized += 1,
            Some(folder) => *counts.folders.entry(folder.clone()).or_default() += 1,
        }
        for tag in &notebook.tags {
            *counts.tags.entry(tag.clone()).or_default() += 1;
        }
    }
    counts
}
