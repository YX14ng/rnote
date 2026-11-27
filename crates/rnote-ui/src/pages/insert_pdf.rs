//! Inserting pages of a PDF between the pages of the notebook shown: its password if it
//! has one, which pages and how, then page by page with progress and Cancel.

use std::ops::Range;
use std::sync::atomic::Ordering;

use adw::prelude::*;
use gettextrs::gettext;
use gtk4::{gio, glib, glib::clone};
use rnote_library::creation::{PdfOpenError, PdfPages, pdf_page_count};
use tracing::error;

use super::RnPagesPanel;
use crate::RnAppWindow;
use crate::library::import_pdf::page_range;
use crate::library::new_notebook::combo;
use crate::library::{dialogs, error_text, progress};

const ACCEPT: &str = "insert";
const CANCEL: &str = "cancel";

impl RnPagesPanel {
    /// Lets the user pick a PDF and inserts its pages before `at` (0-based).
    pub(super) async fn insert_pdf(&self, at: u32) {
        let (Some(appwindow), Some(canvas)) = (self.appwindow(), self.canvas()) else {
            return;
        };
        let filter = gtk4::FileFilter::new();
        filter.set_name(Some(&gettext("PDF")));
        filter.add_suffix("pdf");
        let filters = gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&filter);
        let dialog = gtk4::FileDialog::builder()
            .title(gettext("Insert PDF"))
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let Ok(file) = dialog.open_future(Some(&appwindow)).await else {
            return;
        };
        let bytes = match file.load_contents_future().await {
            Ok((bytes, _)) => bytes.to_vec(),
            Err(e) => {
                error!("Reading the PDF failed, Err: {e:?}");
                appwindow
                    .overlays()
                    .dispatch_toast_error(&error_text::user_message(&e.into()));
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
                    appwindow
                        .overlays()
                        .dispatch_toast_error(&gettext("The PDF can’t be read"));
                    return;
                }
            }
        };
        let Some((pages, vector)) = ask_pages(&appwindow, count).await else {
            return;
        };
        let format = canvas.engine_ref().document.config.format;
        let inserted = progress::run_with_progress(
            &appwindow,
            &gettext("Inserting the PDF"),
            pages.len(),
            move |step, cancelled| {
                let mut pdf = PdfPages::new(bytes, password, pages, vector, format)
                    .map_err(progress::pdf_error)?;
                let mut done = 0;
                while !cancelled.load(Ordering::Relaxed) && pdf.import_next()? {
                    done += 1;
                    step(done);
                }
                if cancelled.load(Ordering::Relaxed) {
                    return Ok(None);
                }
                Ok(Some(pdf.into_pages()))
            },
        )
        .await;
        match inserted {
            Some(Ok(strokes)) => {
                self.edit(|engine| engine.doc_insert_page_strokes_fixed_size(at, strokes));
            }
            Some(Err(e)) => {
                error!("Inserting the PDF failed, Err: {e:?}");
                appwindow
                    .overlays()
                    .dispatch_toast_error(&error_text::user_message(&e));
            }
            None => {}
        }
    }
}

/// Asks which pages of a PDF of `count` pages to insert, and whether as vector images.
/// `None` when cancelled.
async fn ask_pages(appwindow: &RnAppWindow, count: usize) -> Option<(Range<usize>, bool)> {
    let count_f = count as f64;
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
    let kind_row = combo(
        &gettext("Pages Kept As"),
        &[gettext("Vector"), gettext("Images")],
        0,
    );
    let list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    list.append(&first_row);
    list.append(&last_row);
    list.append(&kind_row);
    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Insert PDF"))
        .body(
            gettext("{n} pages: each one goes on its own sheet, underneath to write on.")
                .replace("{n}", &count.to_string()),
        )
        .extra_child(&list)
        .default_response(ACCEPT)
        .close_response(CANCEL)
        .build();
    dialog.add_responses(&[(CANCEL, &gettext("Cancel")), (ACCEPT, &gettext("Insert"))]);
    dialog.set_response_appearance(ACCEPT, adw::ResponseAppearance::Suggested);
    if dialog.choose_future(Some(appwindow)).await != ACCEPT {
        return None;
    }
    let pages = page_range(first_row.value() as u32, last_row.value() as u32, count);
    Some((pages, kind_row.selected() == 0))
}
