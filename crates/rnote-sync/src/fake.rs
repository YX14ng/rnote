//! A Google Drive in memory, for tests. Clones share the same Drive, so several simulated
//! devices can sync with it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use anyhow::anyhow;
use md5::{Digest, Md5};

use crate::remote::{Listing, ROOT_FOLDER_NAME, Remote, RemoteError, RemoteFile, RemoteFolder};

#[derive(Debug, Clone)]
struct Node {
    parent: Option<String>,
    name: String,
    folder: bool,
    bytes: Vec<u8>,
    md5: String,
    modified: String,
    trashed: bool,
}

#[derive(Debug, Default)]
struct Inner {
    nodes: BTreeMap<String, Node>,
    next_id: u64,
    clock: u64,
    ops: usize,
    fail_after: Option<usize>,
    lose_reply: bool,
}

/// A Drive in memory. Like Drive, a folder can hold two files with the same name.
#[derive(Debug, Clone, Default)]
pub struct FakeDrive {
    inner: Rc<RefCell<Inner>>,
}

impl FakeDrive {
    /// An empty Drive.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every operation after the first `ops` ones (counted since the start) fail as
    /// offline.
    pub fn fail_after(&self, ops: usize) {
        let mut inner = self.inner.borrow_mut();
        inner.fail_after = Some(ops);
        inner.lose_reply = false;
    }

    /// Like [`Self::fail_after`], but the first failing operation is done in Drive before
    /// it fails: its reply is lost, as when the connection drops at the wrong moment.
    pub fn lose_reply_after(&self, ops: usize) {
        let mut inner = self.inner.borrow_mut();
        inner.fail_after = Some(ops);
        inner.lose_reply = true;
    }

    /// Stops the failures of [`Self::fail_after`].
    pub fn fail_never(&self) {
        self.inner.borrow_mut().fail_after = None;
    }

    /// How many operations were asked through [`Remote`], failed ones included.
    #[must_use]
    pub fn ops(&self) -> usize {
        self.inner.borrow().ops
    }

    /// The paths of the live files under the `Rnote` folder, sorted (repeated if two
    /// files share a path).
    #[must_use]
    pub fn paths(&self) -> Vec<String> {
        self.paths_where(false)
    }

    /// The paths of the trashed files under the `Rnote` folder, sorted.
    #[must_use]
    pub fn trashed_paths(&self) -> Vec<String> {
        self.paths_where(true)
    }

    /// The content of the live file at `path` under the `Rnote` folder (the first one if
    /// two share it).
    #[must_use]
    pub fn content(&self, path: &str) -> Option<Vec<u8>> {
        let inner = self.inner.borrow();
        let root = inner.root()?;
        inner
            .nodes
            .iter()
            .find(|(_, node)| {
                !node.folder && !node.trashed && inner.path_of(node, &root).as_deref() == Some(path)
            })
            .map(|(_, node)| node.bytes.clone())
    }

    /// Adds a file at `path` under the `Rnote` folder as another client would, creating
    /// the folders on the way. Not counted as an operation.
    pub fn add_file(&self, path: &str, bytes: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        let mut parent = match inner.root() {
            Some(root) => root,
            None => inner.insert(None, ROOT_FOLDER_NAME, true, &[]),
        };
        let mut parts: Vec<&str> = path.split('/').collect();
        let name = parts.pop().unwrap_or_default();
        for part in parts {
            parent = match inner.child_folder(&parent, part) {
                Some(id) => id,
                None => inner.insert(Some(parent), part, true, &[]),
            };
        }
        inner.insert(Some(parent), name, false, bytes);
    }

    /// Sends the `Rnote` folder to the trash, as if the user did it in Drive.
    pub fn trash_root(&self) {
        let mut inner = self.inner.borrow_mut();
        if let Some(root) = inner.root() {
            inner.trash_tree(&root);
        }
    }

    fn paths_where(&self, trashed: bool) -> Vec<String> {
        let inner = self.inner.borrow();
        let Some(root) = inner.root() else {
            return Vec::new();
        };
        let mut paths: Vec<String> = inner
            .nodes
            .values()
            .filter(|node| !node.folder && node.trashed == trashed)
            .filter_map(|node| inner.path_of(node, &root))
            .collect();
        paths.sort();
        paths
    }

    /// Runs the operation `op` as Drive would, failing it as [`Self::fail_after`] and
    /// [`Self::lose_reply_after`] say.
    fn call<T>(
        &self,
        op: impl FnOnce(&mut Inner) -> Result<T, RemoteError>,
    ) -> Result<T, RemoteError> {
        let mut inner = self.inner.borrow_mut();
        inner.ops += 1;
        let offline = || Err(RemoteError::Offline("the fake Drive is offline".to_owned()));
        let Some(limit) = inner.fail_after else {
            return op(&mut inner);
        };
        if inner.ops <= limit {
            return op(&mut inner);
        }
        if inner.lose_reply && inner.ops == limit + 1 {
            // Done in Drive, but the reply never arrives.
            op(&mut inner)?;
        }
        offline()
    }
}

impl Inner {
    fn insert(&mut self, parent: Option<String>, name: &str, folder: bool, bytes: &[u8]) -> String {
        self.next_id += 1;
        let id = format!("id{:06}", self.next_id);
        let node = Node {
            parent,
            name: name.to_owned(),
            folder,
            bytes: bytes.to_vec(),
            md5: md5_hex(bytes),
            modified: self.tick_clock(),
            trashed: false,
        };
        self.nodes.insert(id.clone(), node);
        id
    }

    fn tick_clock(&mut self) -> String {
        self.clock += 1;
        format!("2026-01-01T00:00:00.{:06}Z", self.clock)
    }

    /// The live `Rnote` folder at the top, the oldest if there are several.
    fn root(&self) -> Option<String> {
        self.nodes
            .iter()
            .find(|(_, node)| {
                node.folder
                    && !node.trashed
                    && node.parent.is_none()
                    && node.name == ROOT_FOLDER_NAME
            })
            .map(|(id, _)| id.clone())
    }

    fn child_folder(&self, parent: &str, name: &str) -> Option<String> {
        self.nodes
            .iter()
            .find(|(_, node)| {
                node.folder
                    && !node.trashed
                    && node.name == name
                    && node.parent.as_deref() == Some(parent)
            })
            .map(|(id, _)| id.clone())
    }

    /// The path of `node` relative to the folder `root`, if it's inside it.
    fn path_of(&self, node: &Node, root: &str) -> Option<String> {
        let mut parts = vec![node.name.clone()];
        let mut parent = node.parent.clone();
        while let Some(id) = parent {
            if id == root {
                parts.reverse();
                return Some(parts.join("/"));
            }
            let folder = self.nodes.get(&id)?;
            parts.push(folder.name.clone());
            parent = folder.parent.clone();
        }
        None
    }

    fn trash_tree(&mut self, id: &str) {
        let children: Vec<String> = self
            .nodes
            .iter()
            .filter(|(_, node)| node.parent.as_deref() == Some(id))
            .map(|(child, _)| child.clone())
            .collect();
        for child in children {
            self.trash_tree(&child);
        }
        if let Some(node) = self.nodes.get_mut(id) {
            node.trashed = true;
        }
    }

    fn live_file(&mut self, id: &str) -> Result<&mut Node, RemoteError> {
        self.nodes
            .get_mut(id)
            .filter(|node| !node.folder && !node.trashed)
            .ok_or_else(|| RemoteError::Failed(anyhow!("No file with id {id}")))
    }

    fn remote_file(&self, id: &str) -> RemoteFile {
        let node = &self.nodes[id];
        RemoteFile {
            id: id.to_owned(),
            parent: node.parent.clone().unwrap_or_default(),
            name: node.name.clone(),
            md5: node.md5.clone(),
            modified: node.modified.clone(),
        }
    }
}

impl Remote for FakeDrive {
    fn find_root(&mut self) -> Result<Option<String>, RemoteError> {
        self.call(|inner| Ok(inner.root()))
    }

    fn list(&mut self) -> Result<Listing, RemoteError> {
        self.call(|inner| {
            let mut listing = Listing::default();
            for (id, node) in inner.nodes.iter().filter(|(_, node)| !node.trashed) {
                if node.folder {
                    listing.folders.push(RemoteFolder {
                        id: id.clone(),
                        parent: node.parent.clone(),
                        name: node.name.clone(),
                    });
                } else {
                    listing.files.push(inner.remote_file(id));
                }
            }
            Ok(listing)
        })
    }

    fn download(&mut self, id: &str) -> Result<Vec<u8>, RemoteError> {
        self.call(|inner| Ok(inner.live_file(id)?.bytes.clone()))
    }

    fn create_file(
        &mut self,
        parent: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<RemoteFile, RemoteError> {
        self.call(|inner| {
            let id = inner.insert(Some(parent.to_owned()), name, false, bytes);
            Ok(inner.remote_file(&id))
        })
    }

    fn update_file(&mut self, id: &str, bytes: &[u8]) -> Result<RemoteFile, RemoteError> {
        self.call(|inner| {
            let modified = inner.tick_clock();
            let node = inner.live_file(id)?;
            node.bytes = bytes.to_vec();
            node.md5 = md5_hex(bytes);
            node.modified = modified;
            Ok(inner.remote_file(id))
        })
    }

    fn move_file(
        &mut self,
        id: &str,
        from_parent: &str,
        to_parent: &str,
        name: &str,
    ) -> Result<RemoteFile, RemoteError> {
        self.call(|inner| {
            let node = inner.live_file(id)?;
            if node.parent.as_deref() != Some(from_parent) {
                return Err(RemoteError::Failed(anyhow!("{id} isn't in {from_parent}")));
            }
            node.parent = Some(to_parent.to_owned());
            node.name = name.to_owned();
            Ok(inner.remote_file(id))
        })
    }

    fn create_folder(&mut self, parent: Option<&str>, name: &str) -> Result<String, RemoteError> {
        self.call(|inner| Ok(inner.insert(parent.map(str::to_owned), name, true, &[])))
    }

    fn trash(&mut self, id: &str) -> Result<(), RemoteError> {
        self.call(|inner| {
            if !inner.nodes.contains_key(id) {
                return Err(RemoteError::Failed(anyhow!("No file with id {id}")));
            }
            inner.trash_tree(id);
            Ok(())
        })
    }
}

fn md5_hex(bytes: &[u8]) -> String {
    crate::hex(&Md5::digest(bytes))
}
