//! A PDF imported as a new notebook, one page per call so the app can show progress and
//! cancel between pages. Each page fills the width of the sheet and starts its own sheet;
//! the pages are images under the ink, so the eraser leaves them alone.

use std::ops::Range;
use std::sync::Arc;

use anyhow::anyhow;
use hayro::hayro_syntax::{DecryptionError, LoadPdfError, Pdf};
use parry2d_f64::math::Vector2;
use rnote_compose::Transformable;
use rnote_compose::shapes::Shapeable;
use rnote_engine::document::Layout;
use rnote_engine::engine::import::{PdfImportPageSpacing, PdfImportPrefs};
use rnote_engine::strokes::{BitmapImage, Stroke, VectorImage};
use rnote_engine::{Engine, WidgetFlags};

use crate::notebook_setup::{NotebookSetup, engine_with_setup};

/// The scale of pages imported as images, as the desktop app does.
const BITMAP_SCALE: f64 = 1.8;

/// Why a PDF can't be opened.
#[derive(Debug)]
pub(crate) enum PdfOpenError {
    /// It is encrypted and no password, or a wrong one, was given.
    NeedsPassword,
    /// It can't be read, or the pages asked for aren't in it.
    Invalid(anyhow::Error),
}

/// The number of pages of the PDF `bytes`, opened with `password` if given.
pub(crate) fn pdf_page_count(bytes: &[u8], password: Option<&str>) -> Result<usize, PdfOpenError> {
    let data = Arc::new(bytes.to_vec());
    let pdf = match password {
        Some(password) => Pdf::new_with_password(data, password),
        None => Pdf::new(data),
    };
    match pdf {
        Ok(pdf) => Ok(pdf.pages().len()),
        Err(LoadPdfError::Decryption(DecryptionError::PasswordProtected)) => {
            Err(PdfOpenError::NeedsPassword)
        }
        Err(error) => Err(PdfOpenError::Invalid(anyhow!("The PDF can't be read: {error:?}"))),
    }
}

/// A PDF being imported as a new notebook.
pub(crate) struct PdfImport {
    bytes: Vec<u8>,
    password: Option<String>,
    engine: Engine,
    pages: Range<usize>,
    next: usize,
    vector: bool,
    imported: Vec<Stroke>,
}

impl PdfImport {
    /// Starts importing `pages` (0-based) of the PDF `bytes` into a notebook set up as
    /// `setup` says, as vector images or, if `vector` is false, as bitmaps.
    pub(crate) fn new(
        bytes: Vec<u8>,
        password: Option<String>,
        setup: &NotebookSetup,
        pages: Range<usize>,
        vector: bool,
    ) -> Result<Self, PdfOpenError> {
        let count = pdf_page_count(&bytes, password.as_deref())?;
        if pages.is_empty() || pages.end > count {
            return Err(PdfOpenError::Invalid(anyhow!(
                "Pages {}..{} aren't in a PDF of {count} pages",
                pages.start,
                pages.end
            )));
        }
        Ok(Self {
            bytes,
            password,
            engine: engine_with_setup(setup),
            next: pages.start,
            pages,
            vector,
            imported: Vec::new(),
        })
    }

    /// The number of pages to import.
    pub(crate) fn total(&self) -> usize {
        self.pages.len()
    }

    /// Imports the next page; `false` once every page is imported.
    pub(crate) fn import_next(&mut self) -> anyhow::Result<bool> {
        if self.next >= self.pages.end {
            return Ok(false);
        }
        let prefs = PdfImportPrefs {
            page_width_perc: 100.0,
            page_spacing: PdfImportPageSpacing::OnePerDocumentPage,
            pages_type: Default::default(),
            bitmap_scalefactor: BITMAP_SCALE,
            adjust_document: true,
        };
        let format = &self.engine.document.config.format;
        let range = Some(self.next..self.next + 1);
        let password = self.password.clone();
        let mut page: Vec<Stroke> = if self.vector {
            VectorImage::from_pdf_bytes(&self.bytes, prefs, Vector2::ZERO, range, format, password)?
                .into_iter()
                .map(Stroke::VectorImage)
                .collect()
        } else {
            BitmapImage::from_pdf_bytes(&self.bytes, prefs, Vector2::ZERO, range, format, password)?
                .into_iter()
                .map(Stroke::BitmapImage)
                .collect()
        };
        self.imported.append(&mut page);
        self.next += 1;
        Ok(true)
    }

    /// The `.rnote` bytes of the notebook with the pages imported so far: sheets as tall as
    /// the tallest page, one page on each.
    pub(crate) fn finish(&mut self, file_name: &str) -> anyhow::Result<Vec<u8>> {
        let height = self.imported.iter().map(|page| page.bounds().extents().y).fold(0.0, f64::max);
        if height > 0.0 {
            self.engine.document.config.format.set_height(height);
        }
        let mut flags: WidgetFlags = self.engine.set_doc_layout(Layout::FixedSize);
        let pages = std::mem::take(&mut self.imported)
            .into_iter()
            .enumerate()
            .map(|(i, mut page)| {
                let top = page.bounds().mins.y;
                page.translate(Vector2::new(0.0, i as f64 * height - top));
                (page, None)
            })
            .collect();
        flags |= self.engine.import_generated_content(pages, /* adjust_document */ false);
        flags |= self.engine.deselect_all_strokes();
        flags |= self.engine.doc_resize_to_fit_content();
        let _ = flags;
        futures::executor::block_on(self.engine.save_as_rnote_bytes(file_name.to_owned()))
            .map_err(|_| anyhow!("The save task was cancelled"))?
    }
}

#[cfg(test)]
mod tests;
