//! The files and folders under the `Rnote` folder in Drive, by path relative to it.

use std::collections::BTreeMap;

use crate::local::{is_plain_name, is_synced};
use crate::remote::{Listing, Remote, RemoteError, RemoteFile};

/// What's under the `Rnote` folder, kept up to date as the sync changes Drive.
#[derive(Debug, Clone)]
pub(crate) struct RemoteTree {
    /// Folder path (`""` is the root) → id. Two folders with one path keep the first.
    folders: BTreeMap<String, String>,
    /// File id → path and file.
    files: BTreeMap<String, (String, RemoteFile)>,
}

impl RemoteTree {
    /// The tree of `listing` under the folder `root`; whatever isn't under it is left out.
    pub(crate) fn new(listing: &Listing, root: &str) -> Self {
        let parents: BTreeMap<&str, (Option<&str>, &str)> = listing
            .folders
            .iter()
            .map(|folder| (folder.id.as_str(), (folder.parent.as_deref(), folder.name.as_str())))
            .collect();
        let mut folders = BTreeMap::from([(String::new(), root.to_owned())]);
        let mut paths_by_id = BTreeMap::from([(root.to_owned(), String::new())]);
        for folder in &listing.folders {
            if let Some(path) = folder_path(&parents, &folder.id, root) {
                paths_by_id.insert(folder.id.clone(), path.clone());
                folders.entry(path).or_insert_with(|| folder.id.clone());
            }
        }
        let files = listing
            .files
            .iter()
            .filter_map(|file| {
                let folder = paths_by_id.get(&file.parent)?;
                let path = join(folder, &file.name);
                // Only what the library syncs; a name that could lead outside it is left out.
                is_synced(&path).then(|| (file.id.clone(), (path, file.clone())))
            })
            .collect();
        Self { folders, files }
    }

    /// The files at `path`, oldest first (then by id).
    pub(crate) fn files_at(&self, path: &str) -> Vec<RemoteFile> {
        let mut files: Vec<RemoteFile> = self
            .files
            .values()
            .filter(|(file_path, _)| file_path == path)
            .map(|(_, file)| file.clone())
            .collect();
        files.sort_by(|a, b| (&a.modified, &a.id).cmp(&(&b.modified, &b.id)));
        files
    }

    /// The file `id` and its path, if it's under the root.
    pub(crate) fn file(&self, id: &str) -> Option<(&str, &RemoteFile)> {
        self.files.get(id).map(|(path, file)| (path.as_str(), file))
    }

    /// The paths of all the files.
    pub(crate) fn paths(&self) -> Vec<String> {
        self.files.values().map(|(path, _)| path.clone()).collect()
    }

    /// The paths of all the folders but the root.
    pub(crate) fn folder_paths(&self) -> Vec<String> {
        self.folders.keys().filter(|path| !path.is_empty()).cloned().collect()
    }

    /// The id of the folder at `path` (`""` is the root).
    pub(crate) fn folder_id(&self, path: &str) -> Option<&str> {
        self.folders.get(path).map(String::as_str)
    }

    /// The id of the folder at `path`, creating it and the ones above in Drive if missing.
    pub(crate) fn ensure_folder(
        &mut self,
        remote: &mut dyn Remote,
        path: &str,
    ) -> Result<String, RemoteError> {
        if let Some(id) = self.folders.get(path) {
            return Ok(id.clone());
        }
        let (parent_path, name) = split(path);
        let parent = self.ensure_folder(remote, parent_path)?;
        let id = remote.create_folder(Some(&parent), name)?;
        self.folders.insert(path.to_owned(), id.clone());
        Ok(id)
    }

    /// Records `file` as being at `path` (a new file, or one that changed or moved).
    pub(crate) fn put_file(&mut self, path: &str, file: RemoteFile) {
        self.files.insert(file.id.clone(), (path.to_owned(), file));
    }

    /// Forgets the file `id`.
    pub(crate) fn remove_file(&mut self, id: &str) {
        self.files.remove(id);
    }

    /// Forgets the folder at `path` and everything under it.
    pub(crate) fn remove_folder(&mut self, path: &str) {
        let prefix = format!("{path}/");
        self.folders.retain(|folder, _| folder != path && !folder.starts_with(&prefix));
        self.files.retain(|_, (file, _)| !file.starts_with(&prefix));
    }
}

/// The path of the folder `id` relative to `root`, if it's under it.
fn folder_path(
    parents: &BTreeMap<&str, (Option<&str>, &str)>,
    id: &str,
    root: &str,
) -> Option<String> {
    let mut parts = Vec::new();
    let mut current = id;
    // A cycle can't happen in Drive, but a bad listing mustn't hang the sync.
    for _ in 0..=parents.len() {
        if current == root {
            parts.reverse();
            return Some(parts.join("/"));
        }
        let (parent, name) = parents.get(current)?;
        if !is_plain_name(name) {
            return None;
        }
        parts.push(*name);
        current = (*parent)?;
    }
    None
}

/// `folder/name`, or `name` in the root.
pub(crate) fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() { name.to_owned() } else { format!("{folder}/{name}") }
}

/// The folder path and the name of `path`.
pub(crate) fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}
