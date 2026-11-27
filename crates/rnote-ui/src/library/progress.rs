//! Long work on a thread of its own, behind a dialog with its progress and Cancel.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use futures::StreamExt;
use futures::channel::mpsc;
use gettextrs::gettext;
use gtk4::glib::clone;
use rnote_library::creation::PdfOpenError;

use crate::RnAppWindow;

const CANCEL: &str = "cancel";

/// What the work tells.
enum Step<T> {
    /// This many steps are done.
    Done(usize),
    /// Finished: the result, or `None` if cancelled.
    Finished(anyhow::Result<Option<T>>),
}

/// The error of a PDF that can't be opened.
pub(crate) fn pdf_error(error: PdfOpenError) -> anyhow::Error {
    match error {
        PdfOpenError::NeedsPassword => anyhow::anyhow!("The PDF needs a password"),
        PdfOpenError::Invalid(error) => error,
    }
}

/// Runs `work` on a thread of its own behind a dialog headed `heading`, which shows the
/// progress through `total` steps ("Page 3 of 47") and has Cancel. `work` gets a function
/// to call after each step and the flag Cancel sets; it returns `None` when it stopped
/// because of it. `None` when cancelled.
pub(crate) async fn run_with_progress<T, W>(
    appwindow: &RnAppWindow,
    heading: &str,
    total: usize,
    work: W,
) -> Option<anyhow::Result<T>>
where
    T: Send + 'static,
    W: FnOnce(&dyn Fn(usize), &AtomicBool) -> anyhow::Result<Option<T>> + Send + 'static,
{
    let label = gtk4::Label::new(None);
    let bar = gtk4::ProgressBar::new();
    let hint = gtk4::Label::builder()
        .label(gettext("If you cancel, no half notebook is left."))
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(12)
        .build();
    content.append(&label);
    content.append(&bar);
    content.append(&hint);
    let show = move |done: usize| {
        label.set_label(
            &gettext("Page {n} of {total}")
                .replace("{n}", &(done + 1).min(total).to_string())
                .replace("{total}", &total.to_string()),
        );
        bar.set_fraction(done as f64 / total.max(1) as f64);
    };
    show(0);
    let dialog = adw::AlertDialog::builder()
        .heading(heading)
        .extra_child(&content)
        .close_response(CANCEL)
        .build();
    dialog.add_response(CANCEL, &gettext("Cancel"));
    let cancelled = Arc::new(AtomicBool::new(false));
    dialog.connect_response(
        None,
        clone!(
            #[strong]
            cancelled,
            move |_, _| cancelled.store(true, Ordering::Relaxed)
        ),
    );
    dialog.present(Some(appwindow));

    let (sender, mut receiver) = mpsc::unbounded();
    let stop = cancelled.clone();
    std::thread::spawn(move || {
        let step_sender = sender.clone();
        let step = move |done: usize| {
            let _ = step_sender.unbounded_send(Step::Done(done));
        };
        let result = work(&step, &stop);
        let _ = sender.unbounded_send(Step::Finished(result));
    });
    let mut result = None;
    while let Some(step) = receiver.next().await {
        match step {
            Step::Done(done) => show(done),
            Step::Finished(finished) => {
                result = Some(finished);
                break;
            }
        }
    }
    // Before closing the dialog: closing it answers "cancel".
    let cancelled = cancelled.load(Ordering::Relaxed);
    dialog.force_close();
    match result? {
        Ok(Some(value)) if !cancelled => Some(Ok(value)),
        Ok(_) => None,
        Err(e) => Some(Err(e)),
    }
}
