//! Importing a PDF as a new notebook: its password if it has one, which pages and how,
//! then page by page on a thread of its own, with progress and Cancel. The notebook is
//! written only once every page is in, so cancelling leaves nothing behind.

use std::ops::Range;
use std::sync::atomic::Ordering;

use adw::prelude::*;
use gettextrs::gettext;
use gtk4::{gio, glib, glib::clone};
use rnote_library::creation::{
    NotebookSetup, PAGE_SIZES, PdfImport, PdfOpenError, SetupChoice, pdf_page_count,
};
use tracing::error;

use super::new_notebook::{combo, size_names};
use super::{RnLibraryView, create, dialogs, error_text, import, progress};
use crate::RnAppWindow;

const ACCEPT: &str = "import";
const CANCEL: &str = "cancel";

/// The pages from `first` to `last`, counted from one, of a PDF of `count` pages, as a
/// range counted from zero; pages outside the PDF are left out.
pub(crate) fn page_range(first: u32, last: u32, count: usize) -> Range<usize> {
    let count = count.max(1);
    let first = (first.max(1) as usize).min(count);
    let last = (last as usize).clamp(first, count);
    first - 1..last
}

/// What the import dialog chose.
struct PdfChoice {
    title: String,
    size: usize,
    pages: Range<usize>,
    vector: bool,
}

impl RnLibraryView {
    /// Imports the PDF `file` as a notebook titled `title` in `folder`, then opens it.
    pub(super) async fn import_pdf(&self, file: &gio::File, folder: Option<&str>, title: &str) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let bytes = match import::load(file).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Reading the PDF failed, Err: {e:?}");
                self.toast_error(&error_text::user_message(&e));
                return;
            }
        };
        let mut password: Option<String> = None;
        let count = loop {
            match pdf_page_count(&bytes, password.as_deref()) {
                Ok(count) => break count,
                Err(PdfOpenError::NeedsPassword) => {
                    let wrong = password.is_some();
                    match dialogs::ask_password(&appwindow, wrong).await {
                        Some(typed) => password = Some(typed),
                        None => return,
                    }
                }
                Err(PdfOpenError::Invalid(e)) => {
                    error!("Opening the PDF failed, Err: {e:?}");
                    self.toast_error(&gettext("The PDF can’t be read"));
                    return;
                }
            }
        };
        let mut setup = create::defaults_for(folder);
        let size = SetupChoice::from_setup(&setup).size;
        let Some(choice) = ask_pdf_choice(&appwindow, title, count, size).await else {
            return;
        };
        let page = PAGE_SIZES[choice.size.min(PAGE_SIZES.len() - 1)];
        setup.page_width_mm = page.width_mm;
        setup.page_height_mm = page.height_mm;
        let file_name = format!("{}.rnote", choice.title);
        let Some(notebook) = import_with_progress(
            &appwindow,
            (bytes, password),
            setup,
            choice.pages,
            choice.vector,
            file_name,
        )
        .await
        else {
            return;
        };
        let notebook = match notebook {
            Ok(notebook) => notebook,
            Err(e) => {
                error!("Importing the PDF failed, Err: {e:?}");
                self.toast_error(&error_text::user_message(&e));
                return;
            }
        };
        self.make_notebook(folder, &choice.title, setup.cover, move |_| Ok(notebook))
            .await;
    }
}

/// Asks which pages of a PDF of `count` pages to import, and how. `None` when cancelled.
async fn ask_pdf_choice(
    appwindow: &RnAppWindow,
    title: &str,
    count: usize,
    size: usize,
) -> Option<PdfChoice> {
    let count_f = count as f64;
    let title_row = adw::EntryRow::builder()
        .title(gettext("Title"))
        .text(title)
        .activates_default(true)
        .build();
    let size_row = combo(&gettext("Sheet Width"), &size_names(), size);
    size_row.set_subtitle(&gettext(
        "The height of each sheet follows the PDF’s proportions.",
    ));
    let first_row = adw::SpinRow::with_range(1.0, count_f, 1.0);
    first_row.set_title(&gettext("From Page"));
    let last_row = adw::SpinRow::with_range(1.0, count_f, 1.0);
    last_row.set_title(&gettext("To Page"));
    last_row.set_value(count_f);
    first_row.connect_value_notify(clone!(
        #[weak]
        last_row,
        move |first| {
            if last_row.value() < first.value() {
                last_row.set_value(first.value());
            }
        }
    ));
    last_row.connect_value_notify(clone!(
        #[weak]
        first_row,
        move |last| {
            if first_row.value() > last.value() {
                first_row.set_value(last.value());
            }
        }
    ));
    let kind_row = combo(
        &gettext("Pages Kept As"),
        &[gettext("Vector"), gettext("Images")],
        0,
    );
    kind_row.set_subtitle(&gettext(
        "Vector is lighter and stays sharp at any zoom. The eraser doesn’t erase the PDF: only what you write on top.",
    ));
    let list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&title_row);
    list.append(&size_row);
    list.append(&first_row);
    list.append(&last_row);
    list.append(&kind_row);

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Import PDF"))
        .body(
            gettext("{n} pages: each one goes on its own sheet, underneath to write on.")
                .replace("{n}", &count.to_string()),
        )
        .extra_child(&list)
        .default_response(ACCEPT)
        .close_response(CANCEL)
        .build();
    dialog.add_responses(&[(CANCEL, &gettext("Cancel")), (ACCEPT, &gettext("Import"))]);
    dialog.set_response_appearance(ACCEPT, adw::ResponseAppearance::Suggested);
    if dialog.choose_future(Some(appwindow)).await != ACCEPT {
        return None;
    }
    let title = title_row.text().trim().to_owned();
    Some(PdfChoice {
        title: if title.is_empty() {
            gettext("Untitled")
        } else {
            title
        },
        size: size_row.selected() as usize,
        pages: page_range(first_row.value() as u32, last_row.value() as u32, count),
        vector: kind_row.selected() == 0,
    })
}

/// Imports `pages` of the PDF (`bytes` and its `password`) into a notebook set up as
/// `setup`, showing progress. `None` when cancelled.
async fn import_with_progress(
    appwindow: &RnAppWindow,
    (bytes, password): (Vec<u8>, Option<String>),
    setup: NotebookSetup,
    pages: Range<usize>,
    vector: bool,
    file_name: String,
) -> Option<anyhow::Result<Vec<u8>>> {
    let total = pages.len();
    progress::run_with_progress(
        appwindow,
        &gettext("Importing the PDF"),
        total,
        move |step, cancelled| {
            let mut import = PdfImport::new(bytes, password, &setup, pages, vector)
                .map_err(progress::pdf_error)?;
            let mut done = 0;
            while !cancelled.load(Ordering::Relaxed) && import.import_next()? {
                done += 1;
                step(done);
            }
            if cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            import.finish(&file_name).map(Some)
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::page_range;

    #[test]
    fn pages_counted_from_one_become_a_range_from_zero() {
        assert_eq!(page_range(1, 47, 47), 0..47);
        assert_eq!(page_range(3, 5, 47), 2..5);
    }

    #[test]
    fn pages_outside_the_pdf_are_left_out() {
        assert_eq!(page_range(0, 99, 10), 0..10);
        assert_eq!(page_range(8, 3, 10), 7..8);
    }
}
