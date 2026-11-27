//! Making notebooks: empty ones set up as the "new notebook" dialog chose, from an image
//! or from a PDF, and the setup last used in each folder. Shared by the Android and
//! desktop apps, so both make the same notebooks.

mod defaults;
mod paper;
mod pdf;
mod setup;

#[cfg(test)]
mod tests;

pub use defaults::{defaults_for, remember_defaults};
pub use paper::{apply_background, apply_layout, apply_page_size, color_from_argb};
pub use pdf::{PdfImport, PdfOpenError, PdfPages, pdf_page_count};
pub use setup::{
    DARK_LINES, DARK_PAPER, NotebookSetup, SetupLayout, SetupPattern, engine_with_setup,
    image_notebook_bytes, new_notebook_bytes, upright_png,
};
