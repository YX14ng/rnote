//! The pages of a document of sheets: how many there are, and removing or inserting them.

use rnote_engine::document::format::Format;
use rnote_engine::strokes::Stroke;

use super::{EditState, RnoteDocument};

impl RnoteDocument {
    /// The number of sheets, or `None` for a document without sheets (an infinite note).
    #[must_use]
    pub fn page_count(&self) -> Option<u32> {
        self.engine.doc_page_count_fixed_size()
    }

    /// Whether each sheet has something on it.
    #[must_use]
    pub fn pages_with_content(&self) -> Vec<bool> {
        self.engine.doc_pages_with_content_fixed_size()
    }

    /// Removes the sheets `pages` (0-based) with what is on them; the rest move up. Undoable.
    pub fn remove_pages(&mut self, pages: &[u32]) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.doc_remove_pages_fixed_size(pages);
        self.finish_edit(flags, bounds_before)
    }

    /// Inserts `count` blank sheets before sheet `at` (0-based). Undoable.
    pub fn insert_blank_pages(&mut self, at: u32, count: u32) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.doc_insert_pages_fixed_size(at, count);
        self.finish_edit(flags, bounds_before)
    }

    /// Inserts `pages`, e.g. those of a PDF, each on sheets of its own, before sheet `at`.
    /// Undoable.
    pub fn insert_page_strokes(&mut self, at: u32, pages: Vec<Stroke>) -> EditState {
        let bounds_before = self.bounds();
        let flags = self.engine.doc_insert_page_strokes_fixed_size(at, pages);
        self.finish_edit(flags, bounds_before)
    }
}

impl RnoteDocument {
    /// The size of a sheet, which imported pages are fitted to.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn sheet_format(&self) -> Format {
        self.engine.document.config.format
    }
}
