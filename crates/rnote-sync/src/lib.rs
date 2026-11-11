//! Syncs an Rnote library folder with a folder named `Rnote` in Google Drive.
//!
//! Nothing is ever lost: when a notebook changed on both sides, both versions are kept.
//! Drive is reached through the [`Remote`] trait, so the whole algorithm is tested against
//! an in-memory Drive.

mod conflicts;
mod content;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
mod local;
mod lock;
mod moves;
mod names;
mod remote;
mod run;
mod state;
mod tree;

#[cfg(test)]
mod tests;

use std::path::Path;

use chrono::NaiveDateTime;

pub use crate::remote::{Listing, ROOT_FOLDER_NAME, Remote, RemoteError, RemoteFile, RemoteFolder};
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
    let listing = remote.list()?;
    let known_root =
        state.root_id.clone().filter(|id| listing.folders.iter().any(|folder| &folder.id == id));
    let root = match known_root {
        Some(id) => id,
        None => match remote.find_root()? {
            Some(id) => id,
            None => remote.create_folder(None, ROOT_FOLDER_NAME)?,
        },
    };
    // Another `Rnote` folder: what the state says about Drive no longer holds, and reading
    // it as "deleted in Drive" would delete local notebooks.
    if state.root_id.as_deref() != Some(root.as_str()) {
        state.entries.clear();
        state.root_id = Some(root.clone());
        state.save(ctx.state_path)?;
    }
    let local = local::scan(ctx.library_root)?;
    let tree = RemoteTree::new(&listing, &root);
    let mut run = Run::new(ctx, remote, state, tree, local);
    moves::apply_moves(&mut run)?;
    content::reconcile(&mut run)?;
    content::clean_folders(&mut run)?;
    Ok(run.report)
}

/// `bytes` in lower-case hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
