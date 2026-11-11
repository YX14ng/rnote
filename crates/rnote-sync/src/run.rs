//! One sync in progress and the operations it's made of. Each operation saves the state
//! right after it, so a sync cut in the middle neither repeats nor loses work, and each one
//! that could overwrite or remove something first checks that it's still what the sync saw:
//! an editor may save here, and another device may sync, while this sync runs.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{Context, anyhow};

use crate::local::{hash_now, set_aside, sha256_hex, write_atomic};
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

    /// Fails unless the local `path` is still as the scan saw it (or still missing).
    pub(crate) fn expect_local(&self, path: &str) -> Result<(), SyncError> {
        if hash_now(self.ctx.library_root, path)?.as_ref() == self.local.get(path) {
            Ok(())
        } else {
            Err(SyncError::Failed(anyhow!("{path} changed here during the sync")))
        }
    }

    /// Writes `bytes` to the local `path`, if nothing changed it since the scan. Returns
    /// their hash.
    pub(crate) fn write_local(&mut self, path: &str, bytes: &[u8]) -> Result<String, SyncError> {
        self.expect_local(path)?;
        write_atomic(self.ctx.library_root, path, bytes)?;
        let sha256 = sha256_hex(bytes);
        self.local.insert(path.to_owned(), sha256.clone());
        self.report.changed_here.push(path.to_owned());
        Ok(sha256)
    }

    /// Fails unless the Drive file `id` is still as the listing showed it.
    fn expect_remote(&mut self, id: &str) -> Result<Option<RemoteFile>, SyncError> {
        let listed = self.tree.file(id).map(|(_, file)| file.md5.clone());
        let now = self.remote.get(id)?;
        match (&now, listed) {
            (Some(file), Some(md5)) if file.md5 != md5 => {
                Err(SyncError::Failed(anyhow!("{} changed in Drive during the sync", file.name)))
            }
            _ => Ok(now),
        }
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

    /// Uploads the local `path` over the Drive file `id`, if Drive still has what the
    /// listing showed. Returns the content of another device's version that was replaced
    /// meanwhile, if that happened between the check and the upload.
    pub(crate) fn upload_update(
        &mut self,
        path: &str,
        id: &str,
    ) -> Result<Option<Vec<u8>>, SyncError> {
        let expected = self.tree.file(id).map(|(_, file)| file.md5.clone());
        if self.expect_remote(id)?.is_none() {
            return Err(SyncError::Failed(anyhow!("{path} was removed in Drive during the sync")));
        }
        let bytes = self.read_local(path)?;
        let file = self.remote.update_file(id, &bytes)?;
        self.tree.put_file(path, file.clone());
        self.report.uploaded += 1;
        self.record(path, &file, sha256_hex(&bytes))?;
        let Some(expected) = expected else {
            return Ok(None);
        };
        // The version just before ours must be the one this sync saw.
        let revisions = self.remote.revisions(id)?;
        let Some(ours) = revisions.iter().rposition(|revision| revision.md5 == file.md5) else {
            return Ok(None);
        };
        match ours.checked_sub(1).map(|index| &revisions[index]) {
            Some(previous) if previous.md5 != expected => {
                Ok(Some(self.remote.download_revision(id, &previous.id)?))
            }
            _ => Ok(None),
        }
    }

    /// Downloads `file` to the local `path`.
    pub(crate) fn download(&mut self, path: &str, file: &RemoteFile) -> Result<(), SyncError> {
        let bytes = self.remote.download(&file.id)?;
        let sha256 = self.write_local(path, &bytes)?;
        self.report.downloaded += 1;
        self.record(path, file, sha256)
    }

    /// Sends the Drive file `id`, at `path`, to Drive's trash, if Drive still has what the
    /// listing showed.
    pub(crate) fn trash_remote(&mut self, path: &str, id: &str) -> Result<(), SyncError> {
        if self.expect_remote(id)?.is_some() {
            self.remote.trash(id)?;
            self.report.deleted += 1;
        }
        self.tree.remove_file(id);
        self.forget(path)
    }

    /// Sends a second Drive file at the same path, with the same content as the one kept,
    /// to Drive's trash.
    pub(crate) fn trash_remote_duplicate(&mut self, id: &str) -> Result<(), SyncError> {
        self.remote.trash(id)?;
        self.tree.remove_file(id);
        Ok(())
    }

    /// Takes the local `path` out of the library, if nothing changed it since the scan. It
    /// isn't deleted: it's set aside in a hidden folder that isn't synced.
    pub(crate) fn delete_local(&mut self, path: &str) -> Result<(), SyncError> {
        self.expect_local(path)?;
        let stamp = self.ctx.now.format("%Y-%m-%d %H.%M").to_string();
        set_aside(self.ctx.library_root, path, &stamp)?;
        self.local.remove(path);
        self.emptied.insert(split(path).0.to_owned());
        self.report.changed_here.push(path.to_owned());
        self.report.deleted += 1;
        self.forget(path)
    }
}
