//! The cards of the library: one per notebook, plus "New" first.

use std::path::Path;

use gettextrs::gettext;
use gtk4::{Align, Orientation, gio, glib, pango, prelude::*};
use rnote_library::{Notebook, NotebookKind, TrashEntry};

use super::thumbnails::{THUMBNAIL_WIDTH, Thumbnails};

/// The height of a card's thumbnail: an A4 page at [`THUMBNAIL_WIDTH`].
const THUMBNAIL_HEIGHT: i32 = THUMBNAIL_WIDTH * 297 / 210;
/// The thumbnail size in the list view.
const LIST_THUMBNAIL_WIDTH: i32 = 48;
const LIST_THUMBNAIL_HEIGHT: i32 = LIST_THUMBNAIL_WIDTH * 297 / 210;
/// The width of a cover's spine, and where its name label goes, as fractions of the cover.
const COVER_SPINE: f64 = 0.08;
const COVER_LABEL_LEFT: f64 = 0.16;
const COVER_LABEL_RIGHT: f64 = 0.10;
const COVER_LABEL_TOP: f64 = 0.39;
const COVER_LABEL_HEIGHT: f64 = 0.23;

/// A card: a button that opens it, with its menu button over it (not inside it).
#[derive(Debug)]
pub(crate) struct Card {
    /// The card itself.
    pub(crate) widget: gtk4::Widget,
    /// The button that opens the card.
    pub(crate) button: gtk4::Button,
    /// The button with the card's menu; hidden until it gets one.
    pub(crate) menu_button: gtk4::MenuButton,
    /// The check of the card in selection mode; hidden otherwise.
    pub(crate) check: gtk4::CheckButton,
}

impl Card {
    /// Gives the card `menu`.
    pub(crate) fn set_menu(&self, menu: &gio::MenuModel) {
        self.menu_button.set_menu_model(Some(menu));
        self.menu_button.set_visible(true);
    }

    /// Shows the selection check instead of the menu, checked when `selected`.
    pub(crate) fn set_selectable(&self, selected: bool) {
        self.check.set_active(selected);
        self.check.set_visible(true);
        self.menu_button.set_visible(false);
    }
}

/// The cover of a card: a plain page of a fixed size with `child` filling it. The child is
/// an overlay, so a large thumbnail never makes the card grow.
fn cover(child: &impl IsA<gtk4::Widget>, list: bool) -> gtk4::Overlay {
    let sizer = gtk4::Box::new(Orientation::Vertical, 0);
    if list {
        sizer.set_size_request(LIST_THUMBNAIL_WIDTH, LIST_THUMBNAIL_HEIGHT);
    } else {
        sizer.set_size_request(THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
    }
    let cover = gtk4::Overlay::builder()
        .child(&sizer)
        .halign(Align::Center)
        .overflow(gtk4::Overflow::Hidden)
        .build();
    cover.add_css_class("card");
    cover.add_overlay(child);
    cover
}

/// Lays out a card: the cover, then the title and subtitle, and the menu button.
fn layout(cover: &gtk4::Overlay, title: &str, subtitle: &str, list: bool) -> Card {
    let title_label = gtk4::Label::builder()
        .label(title)
        .xalign(0.0)
        .ellipsize(pango::EllipsizeMode::Middle)
        .max_width_chars(20)
        .build();
    title_label.add_css_class("heading");
    let subtitle_label = gtk4::Label::builder()
        .label(subtitle)
        .xalign(0.0)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    subtitle_label.add_css_class("caption");
    subtitle_label.add_css_class("dim-label");
    let labels = gtk4::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(Align::Center)
        // Room for the menu button laid over the card.
        .margin_end(36)
        .build();
    labels.append(&title_label);
    labels.append(&subtitle_label);

    let content = if list {
        let row = gtk4::Box::builder().spacing(12).build();
        row.append(cover);
        row.append(&labels);
        row
    } else {
        labels.set_width_request(THUMBNAIL_WIDTH - 36);
        let column = gtk4::Box::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .build();
        column.append(cover);
        column.append(&labels);
        column
    };
    let button = gtk4::Button::builder()
        .child(&content)
        .tooltip_text(title)
        .build();
    button.add_css_class("flat");
    button.update_property(&[gtk4::accessible::Property::Label(title)]);

    let menu_button = gtk4::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .halign(Align::End)
        .valign(Align::End)
        .margin_end(6)
        .margin_bottom(if list { 12 } else { 8 })
        .visible(false)
        .tooltip_text(gettext("Notebook Menu"))
        .build();
    menu_button.add_css_class("flat");
    let check = gtk4::CheckButton::builder()
        .halign(Align::Start)
        .valign(Align::Start)
        .margin_start(10)
        .margin_top(10)
        .visible(false)
        .tooltip_text(gettext("Select"))
        .build();
    check.add_css_class("selection-mode");
    let widget = gtk4::Overlay::builder().child(&button).build();
    widget.add_overlay(&menu_button);
    widget.add_overlay(&check);
    Card {
        widget: widget.upcast(),
        button,
        menu_button,
        check,
    }
}

/// A cover with a dim `icon_name` icon.
fn icon_cover(icon_name: &str, list: bool) -> gtk4::Overlay {
    let icon = gtk4::Image::builder()
        .icon_name(icon_name)
        .pixel_size(if list { 24 } else { 48 })
        .hexpand(true)
        .halign(Align::Center)
        .valign(Align::Center)
        .build();
    icon.add_css_class("dim-label");
    cover(&icon, list)
}

/// The color (red, green, blue and alpha, from 0 to 1) of a cover stored as ARGB.
fn cover_rgba(argb: u32) -> [f64; 4] {
    let channel = |shift: u32| f64::from((argb >> shift) & 0xFF) / 255.0;
    [channel(16), channel(8), channel(0), channel(24)]
}

/// A notebook cover of the color `argb`: a darker spine and, in the grid, `name` on a
/// light label, as the Android library shows it.
fn color_cover(argb: u32, name: &str, list: bool) -> gtk4::Overlay {
    let [red, green, blue, alpha] = cover_rgba(argb);
    let area = gtk4::DrawingArea::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    area.set_draw_func(move |_, cr, width, height| {
        let (width, height) = (f64::from(width), f64::from(height));
        cr.set_source_rgba(red, green, blue, alpha);
        cr.rectangle(0.0, 0.0, width, height);
        let _ = cr.fill();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.18);
        cr.rectangle(0.0, 0.0, width * COVER_SPINE, height);
        let _ = cr.fill();
        if !list {
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.86);
            cr.rectangle(
                width * COVER_LABEL_LEFT,
                height * COVER_LABEL_TOP,
                width * (1.0 - COVER_LABEL_LEFT - COVER_LABEL_RIGHT),
                height * COVER_LABEL_HEIGHT,
            );
            let _ = cr.fill();
        }
    });
    let overlay = cover(&area, list);
    if !list {
        let width = f64::from(THUMBNAIL_WIDTH);
        let height = f64::from(THUMBNAIL_HEIGHT);
        let label = gtk4::Label::builder()
            .use_markup(true)
            .label(format!(
                "<span foreground=\"#1B1C21\" weight=\"bold\">{}</span>",
                glib::markup_escape_text(name)
            ))
            .xalign(0.0)
            .ellipsize(pango::EllipsizeMode::End)
            .valign(Align::Start)
            .halign(Align::Fill)
            .margin_top((height * COVER_LABEL_TOP) as i32)
            .height_request((height * COVER_LABEL_HEIGHT) as i32)
            .margin_start((width * COVER_LABEL_LEFT) as i32 + 8)
            .margin_end((width * COVER_LABEL_RIGHT) as i32 + 8)
            .build();
        overlay.add_overlay(&label);
    }
    overlay
}

/// The "New" card, which creates a notebook.
pub(crate) fn new_card(list: bool) -> Card {
    let card = layout(
        &icon_cover("list-add-symbolic", list),
        &gettext("New"),
        &gettext("Notebook"),
        list,
    );
    card.widget.set_tooltip_text(Some(&gettext("New Notebook")));
    card
}

/// The card of `notebook` of the library at `root`, with its thumbnail loading.
pub(crate) fn notebook_card(
    notebook: &Notebook,
    root: &Path,
    thumbnails: &Thumbnails,
    list: bool,
) -> Card {
    let cover = match notebook.kind {
        NotebookKind::Rnote if notebook.cover.is_some() => {
            color_cover(notebook.cover.unwrap_or_default(), &notebook.name, list)
        }
        NotebookKind::Rnote => {
            let picture = gtk4::Picture::builder()
                .content_fit(gtk4::ContentFit::Cover)
                .can_shrink(true)
                .hexpand(true)
                .build();
            thumbnails.load(&picture, root.to_owned(), notebook);
            cover(&picture, list)
        }
        NotebookKind::Xopp => icon_cover("x-office-document-symbolic", list),
    };
    let subtitle = match notebook.kind {
        NotebookKind::Rnote => format_date(notebook.modified_ms),
        NotebookKind::Xopp => gettext("Xournal++ · import"),
    };
    layout(&cover, &notebook.name, &subtitle, list)
}

/// The card of a notebook in the trash.
pub(crate) fn trash_card(entry: &TrashEntry, list: bool) -> Card {
    let subtitle = gettext("Deleted {date}").replace("{date}", &format_date(entry.deleted_at_ms));
    layout(
        &icon_cover("user-trash-symbolic", list),
        &entry.name,
        &subtitle,
        list,
    )
}

/// A date in the user's format, from milliseconds since the Unix epoch.
pub(crate) fn format_date(ms: i64) -> String {
    glib::DateTime::from_unix_local(ms.div_euclid(1000))
        .and_then(|date| date.format("%x"))
        .map(|date| date.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::cover_rgba;

    #[test]
    fn cover_rgba_uses_the_argb_color() {
        assert_eq!(
            cover_rgba(0xFF3D_6FB6),
            [61.0 / 255.0, 111.0 / 255.0, 182.0 / 255.0, 1.0]
        );
        let half = cover_rgba(0x803F_8F6B);
        assert_eq!(half[..3], [63.0 / 255.0, 143.0 / 255.0, 107.0 / 255.0]);
        assert!((half[3] - 0.5).abs() < 0.01, "{half:?}");
    }
}
