//! Saving without holding up the document: the document as it is now is taken at once, and
//! made into the file's bytes elsewhere while drawing goes on.

use anyhow::Context;
use futures::channel::oneshot;

use super::RnoteDocument;

/// A save under way: the document as it was when the save began, being made into bytes.
pub struct PendingSave {
    bytes: oneshot::Receiver<anyhow::Result<Vec<u8>>>,
    edits: u64,
}

impl PendingSave {
    /// The changes the document had when the save began, for
    /// [`RnoteDocument::mark_saved_as_of`].
    #[must_use]
    pub fn edits(&self) -> u64 {
        self.edits
    }

    /// The `.rnote` bytes, once they are made; blocks until then.
    pub fn wait(self) -> anyhow::Result<Vec<u8>> {
        futures::executor::block_on(self.bytes).context("The save task was cancelled")?
    }
}

impl RnoteDocument {
    /// Starts saving the document as an `.rnote` named `file_name`: takes it as it is now,
    /// and makes the bytes on another thread. The document stays modified until
    /// [`Self::mark_saved_as_of`] is called once the bytes are written.
    pub fn begin_save(&self, file_name: &str) -> PendingSave {
        PendingSave {
            bytes: self.engine.save_as_rnote_bytes(file_name.to_owned()),
            edits: self.edits,
        }
    }

    /// Marks the document as saved if it hasn't changed since the save that saw `edits`
    /// changes began: what changed meanwhile is still unsaved.
    pub fn mark_saved_as_of(&mut self, edits: u64) {
        if self.edits == edits {
            self.modified = false;
        }
    }
}
