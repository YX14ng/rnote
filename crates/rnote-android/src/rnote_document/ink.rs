//! The writing of the document, for the ink index.

use super::RnoteDocument;
use crate::ink_index::fragments::{self, InkStroke};

impl RnoteDocument {
    /// The pen strokes, in the order they were drawn.
    // TEMPORARY until the JNI bridge (stage 6, task 3) uses it.
    #[allow(dead_code)]
    pub(crate) fn ink_strokes(&self) -> Vec<InkStroke> {
        fragments::ink_strokes(&self.engine)
    }
}
