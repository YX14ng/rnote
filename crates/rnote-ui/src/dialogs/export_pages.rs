//! The pages of a notebook of sheets that go in a PDF export: all of them, the one on screen,
//! or a range, as on Android.

use adw::prelude::*;
use gettextrs::gettext;
use gtk4::{Builder, glib, glib::clone};
use rnote_engine::engine::export::DocExportFormat;

use crate::{RnAppWindow, RnCanvas, RnStrokeContentPreview};

/// Which pages the PDF takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    All,
    Current,
    Range,
}

/// The pages (from 0, both ends included) of a notebook of `count` pages that `choice`
/// takes: none chosen for all of them, the `current` one, or those from `from` to `to`
/// (numbers as shown, from 1) in either order, kept inside the notebook.
fn chosen_pages(
    choice: Choice,
    current: u32,
    from: u32,
    to: u32,
    count: u32,
) -> Option<(u32, u32)> {
    let last = count.saturating_sub(1);
    match choice {
        Choice::All => None,
        Choice::Current => Some((current.min(last), current.min(last))),
        Choice::Range => {
            let first = from.min(to).saturating_sub(1).min(last);
            Some((first, from.max(to).saturating_sub(1).min(last)))
        }
    }
}

/// Wires the rows of the export dialog built by `builder` that choose the pages, for the
/// notebook `canvas` shows; `preview` shows the pages chosen. Only a PDF of a notebook of
/// sheets takes some pages: otherwise the rows stay hidden.
pub(crate) fn wire(
    builder: &Builder,
    dialog: &adw::Dialog,
    appwindow: &RnAppWindow,
    canvas: &RnCanvas,
    preview: &RnStrokeContentPreview,
) {
    let pages_row: adw::ComboRow = builder.object("export_doc_pages_row").unwrap();
    let from_row: adw::SpinRow = builder.object("export_doc_pages_from_row").unwrap();
    let to_row: adw::SpinRow = builder.object("export_doc_pages_to_row").unwrap();
    let format_row: adw::ComboRow = builder.object("export_doc_export_format_row").unwrap();
    let blank_pages_row: adw::SwitchRow =
        builder.object("export_doc_with_empty_pages_row").unwrap();
    // The pages chosen are for this export only.
    forget_chosen_pages(appwindow);
    dialog.connect_closed(clone!(
        #[weak]
        appwindow,
        move |_| forget_chosen_pages(&appwindow)
    ));
    let (count, current) = {
        let engine = canvas.engine_ref();
        let current = engine.doc_page_at_fixed_size(engine.camera.viewport_center()[1]);
        (engine.doc_page_count_fixed_size(), current.unwrap_or(0))
    };
    let Some(count) = count.filter(|count| *count > 1) else {
        pages_row.set_visible(false);
        return;
    };
    let labels = [
        gettext("All Pages"),
        gettext("This Page ({n})").replace("{n}", &(current + 1).to_string()),
        gettext("Choose…"),
    ];
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    pages_row.set_model(Some(&gtk4::StringList::new(&labels)));
    for row in [&from_row, &to_row] {
        row.set_range(1.0, f64::from(count));
    }
    from_row.set_value(1.0);
    to_row.set_value(f64::from(count));

    let update = clone!(
        #[weak]
        pages_row,
        #[weak]
        from_row,
        #[weak]
        to_row,
        #[weak]
        format_row,
        #[weak]
        blank_pages_row,
        #[weak]
        appwindow,
        #[weak]
        canvas,
        #[weak]
        preview,
        move || {
            let pdf = matches!(
                DocExportFormat::try_from(format_row.selected()),
                Ok(DocExportFormat::Pdf)
            );
            pages_row.set_sensitive(pdf);
            let choice = match pages_row.selected() {
                1 => Choice::Current,
                2 => Choice::Range,
                _ => Choice::All,
            };
            // Some pages are taken blank or not.
            blank_pages_row.set_sensitive(pdf && choice == Choice::All);
            let range = choice == Choice::Range;
            from_row.set_visible(pdf && range);
            to_row.set_visible(pdf && range);
            let pages = if pdf {
                chosen_pages(
                    choice,
                    current,
                    from_row.value() as u32,
                    to_row.value() as u32,
                    count,
                )
            } else {
                None
            };
            let prefs = {
                let mut config = appwindow.engine_config().write();
                config.export_prefs.doc_export_prefs.pages = pages;
                config.export_prefs.doc_export_prefs
            };
            preview.set_contents(canvas.engine_ref().export_pages_content(&prefs));
        }
    );
    pages_row.connect_selected_notify(clone!(
        #[strong]
        update,
        move |_| update()
    ));
    format_row.connect_selected_notify(clone!(
        #[strong]
        update,
        move |_| update()
    ));
    from_row.connect_value_notify(clone!(
        #[strong]
        update,
        move |_| update()
    ));
    to_row.connect_value_notify(move |_| update());
    glib::idle_add_local_once(clone!(
        #[weak]
        pages_row,
        move || pages_row.notify("selected")
    ));
}

fn forget_chosen_pages(appwindow: &RnAppWindow) {
    appwindow
        .engine_config()
        .write()
        .export_prefs
        .doc_export_prefs
        .pages = None;
}

#[cfg(test)]
mod tests {
    use super::{Choice, chosen_pages};

    #[test]
    fn the_pages_chosen_are_kept_in_order_and_inside_the_notebook() {
        assert_eq!(chosen_pages(Choice::All, 2, 1, 5, 5), None);
        assert_eq!(chosen_pages(Choice::Current, 2, 1, 5, 5), Some((2, 2)));
        // Numbers as shown (from 1), pages from 0, in either order.
        assert_eq!(chosen_pages(Choice::Range, 0, 4, 2, 5), Some((1, 3)));
        assert_eq!(chosen_pages(Choice::Range, 0, 3, 9, 5), Some((2, 4)));
    }
}
