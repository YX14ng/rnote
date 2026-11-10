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
