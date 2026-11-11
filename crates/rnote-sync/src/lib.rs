//! Syncs an Rnote library folder with a folder named `Rnote` in Google Drive.
//!
//! Nothing is ever lost: when a notebook changed on both sides, both versions are kept.
//! Drive is reached through the [`Remote`] trait, so the whole algorithm is tested against
//! an in-memory Drive.

mod conflicts;
mod content;
mod drive;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
mod local;
mod lock;
mod moves;
mod names;
mod remote;
mod revoke;
mod run;
mod state;
mod tree;

#[cfg(test)]
mod tests;

use std::path::Path;

use chrono::NaiveDateTime;

pub use crate::drive::DriveClient;
pub use crate::names::conflict_copy_path;
pub use crate::remote::{
    Listing, ROOT_FOLDER_NAME, Remote, RemoteError, RemoteFile, RemoteFolder, Revision,
};
pub use crate::revoke::{revoke, revoke_at};
use crate::run::Run;
use crate::state::SyncState;
use crate::tree::RemoteTree;

/// What a sync works on.
#[derive(Debug, Clone, Copy)]
pub struct SyncContext<'a> {
    /// The library folder.
    pub library_root: &'a Path,
    /// Where this device keeps its sync state, outside the library.
    pub state_path: &'a Path,
    /// The name of this device, for the names of conflict copies.
    pub device: &'a str,
    /// The local time, for the names of conflict copies.
    pub now: NaiveDateTime,
}

/// What a sync did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Files sent to Drive.
    pub uploaded: usize,
    /// Files brought from Drive.
    pub downloaded: usize,
    /// Files moved or renamed, on either side.
    pub moved: usize,
    /// Files deleted here or sent to Drive's trash.
    pub deleted: usize,
    /// The local paths of the conflict copies made.
    pub conflicts: Vec<String>,
    /// The local paths this sync wrote, moved or removed, so open notebooks can be
    /// reloaded or protected.
    pub changed_here: Vec<String>,
    /// The paths that couldn't be synced this time, each with the reason; the next sync
    /// tries them again.
    pub skipped: Vec<String>,
}

/// Why a sync stopped. What it did until then is kept.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// Another sync of this library is running.
    #[error("A sync is already running")]
    Busy,
    /// The access token was refused: sign in again.
    #[error("Google Drive refused the access token")]
    Unauthorized,
    /// Drive can't be reached; try again later.
    #[error("Google Drive can't be reached: {0}")]
    Offline(String),
    /// Anything else.
    #[error(transparent)]
    Failed(#[from] anyhow::Error),
}

impl From<RemoteError> for SyncError {
    fn from(error: RemoteError) -> Self {
        match error {
            RemoteError::Unauthorized => Self::Unauthorized,
            RemoteError::Offline(reason) => Self::Offline(reason),
            RemoteError::Failed(error) => Self::Failed(error),
        }
    }
}

/// Syncs the library at `ctx.library_root` with the `Rnote` folder of `remote`.
pub fn sync(ctx: &SyncContext, remote: &mut dyn Remote) -> Result<SyncReport, SyncError> {
    let Some(_lock) = lock::acquire(ctx.state_path)? else {
        return Err(SyncError::Busy);
    };
    let mut state = SyncState::load(ctx.state_path)?;
    let library_id = local::library_id(ctx.library_root)?;
    let listing = remote.list()?;
    let root = match oldest_root(&listing) {
        Some(id) => id,
        None => match remote.find_root()? {
            Some(id) => id,
            None => remote.create_folder(None, ROOT_FOLDER_NAME)?,
        },
    };
    // Another `Rnote` folder, or another library folder here (changed in the settings,
    // replaced, or a disk that isn't mounted): what the state says no longer holds, and
    // reading it as "deleted" would delete notebooks. Both sides are compared afresh.
    if state.root_id.as_deref() != Some(root.as_str())
        || state.library_id.as_deref() != Some(library_id.as_str())
    {
        state.entries.clear();
        state.root_id = Some(root.clone());
        state.library_id = Some(library_id);
        state.save(ctx.state_path)?;
    }
    let local = local::scan(ctx.library_root)?;
    let tree = RemoteTree::new(&listing, &root);
    let mut run = Run::new(ctx, remote, state, tree, local);
    moves::apply_moves(&mut run)?;
    content::reconcile(&mut run)?;
    content::clean_folders(&mut run)?;
    let mut report = run.report;
    report.changed_here.sort();
    report.changed_here.dedup();
    Ok(report)
}

/// The oldest `Rnote` folder at the top of what the app sees: every device picks the same
/// one, even if two created one at the same time.
fn oldest_root(listing: &Listing) -> Option<String> {
    let ids: std::collections::BTreeSet<&str> =
        listing.folders.iter().map(|folder| folder.id.as_str()).collect();
    listing
        .folders
        .iter()
        .filter(|folder| folder.name == ROOT_FOLDER_NAME)
        .filter(|folder| folder.parent.as_deref().is_none_or(|parent| !ids.contains(parent)))
        .min_by(|a, b| (&a.created, &a.id).cmp(&(&b.created, &b.id)))
        .map(|folder| folder.id.clone())
}

/// `bytes` in lower-case hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
