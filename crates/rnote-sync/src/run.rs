//! One sync in progress and the operations it's made of. Each operation saves the state
//! right after it, so a sync cut in the middle neither repeats nor loses work.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::Context;

use crate::local::{sha256_hex, write_atomic};
use crate::remote::{Remote, RemoteFile};
use crate::state::{Entry, SyncState};
use crate::tree::{RemoteTree, split};
use crate::{SyncContext, SyncError, SyncReport};

/// A sync in progress: what's known of both sides, updated as it goes.
pub(crate) struct Run<'a> {
    pub ctx: &'a SyncContext<'a>,
    pub remote: &'a mut dyn Remote,
    pub state: SyncState,
    pub tree: RemoteTree,
    /// The synced local files: path → SHA-256.
    pub local: BTreeMap<String, String>,
    pub report: SyncReport,
    /// Local folders this sync took a file out of.
    pub emptied: BTreeSet<String>,
}

impl<'a> Run<'a> {
    pub(crate) fn new(
        ctx: &'a SyncContext<'a>,
        remote: &'a mut dyn Remote,
        state: SyncState,
        tree: RemoteTree,
        local: BTreeMap<String, String>,
    ) -> Self {
        Self {
            ctx,
            remote,
            state,
            tree,
            local,
            report: SyncReport::default(),
            emptied: BTreeSet::new(),
        }
    }

    pub(crate) fn save(&self) -> Result<(), SyncError> {
        Ok(self.state.save(self.ctx.state_path)?)
    }

    /// Notes that `path` is `file` on both sides, with local content of hash `sha256`.
    pub(crate) fn record(
        &mut self,
        path: &str,
        file: &RemoteFile,
        sha256: String,
    ) -> Result<(), SyncError> {
        let entry = Entry {
            id: file.id.clone(),
            md5: file.md5.clone(),
            modified: file.modified.clone(),
            sha256,
        };
        self.state.entries.insert(path.to_owned(), entry);
        self.save()
    }

    /// Forgets `path` in the state.
    pub(crate) fn forget(&mut self, path: &str) -> Result<(), SyncError> {
        if self.state.entries.remove(path).is_some() {
            self.save()?;
        }
        Ok(())
    }

    pub(crate) fn read_local(&self, path: &str) -> Result<Vec<u8>, SyncError> {
        Ok(fs::read(self.ctx.library_root.join(path)).with_context(|| format!("Reading {path}"))?)
    }

    /// Writes `bytes` to the local `path`. Returns their hash.
    pub(crate) fn write_local(&mut self, path: &str, bytes: &[u8]) -> Result<String, SyncError> {
        write_atomic(self.ctx.library_root, path, bytes)?;
        let sha256 = sha256_hex(bytes);
        self.local.insert(path.to_owned(), sha256.clone());
        Ok(sha256)
    }

    /// Uploads the local `path` as a new file in Drive, at the same path.
    pub(crate) fn upload_new(&mut self, path: &str) -> Result<(), SyncError> {
        let bytes = self.read_local(path)?;
        let (folder, name) = split(path);
        let parent = self.tree.ensure_folder(self.remote, folder)?;
        let file = self.remote.create_file(&parent, name, &bytes)?;
        self.tree.put_file(path, file.clone());
        self.report.uploaded += 1;
        self.record(path, &file, sha256_hex(&bytes))
    }

    /// Uploads the local `path` over the Drive file `id`.
    pub(crate) fn upload_update(&mut self, path: &str, id: &str) -> Result<(), SyncError> {
        let bytes = self.read_local(path)?;
        let file = self.remote.update_file(id, &bytes)?;
        self.tree.put_file(path, file.clone());
        self.report.uploaded += 1;
        self.record(path, &file, sha256_hex(&bytes))
    }

    /// Downloads `file` to the local `path`.
    pub(crate) fn download(&mut self, path: &str, file: &RemoteFile) -> Result<(), SyncError> {
        let bytes = self.remote.download(&file.id)?;
        let sha256 = self.write_local(path, &bytes)?;
        self.report.downloaded += 1;
        self.record(path, file, sha256)
    }

    /// Sends the Drive file `id`, at `path`, to Drive's trash.
    pub(crate) fn trash_remote(&mut self, path: &str, id: &str) -> Result<(), SyncError> {
        self.remote.trash(id)?;
        self.tree.remove_file(id);
        self.report.deleted += 1;
        self.forget(path)
    }

    /// Deletes the local `path`.
    pub(crate) fn delete_local(&mut self, path: &str) -> Result<(), SyncError> {
        let file = self.ctx.library_root.join(path);
        fs::remove_file(&file).with_context(|| format!("Deleting {path}"))?;
        self.local.remove(path);
        self.emptied.insert(split(path).0.to_owned());
        self.report.deleted += 1;
        self.forget(path)
    }
}
