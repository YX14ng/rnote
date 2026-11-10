//! The library's sidebar: its sections with the number of notebooks in each.

use gettextrs::gettext;
use gtk4::{Align, gdk, prelude::*};
use rnote_library::{Section, SectionCounts, Tag};

/// The title of `section`, given the library's `tags`.
pub(crate) fn section_title(section: &Section, tags: &[Tag]) -> String {
    match section {
        Section::All => gettext("All Notes"),
        Section::Uncategorized => gettext("Uncategorized"),
        Section::Folder(folder) => folder.clone(),
        Section::Tag(id) => tags
            .iter()
            .find(|tag| &tag.id == id)
            .map(|tag| tag.name.clone())
            .unwrap_or_default(),
        Section::Trash => gettext("Trash"),
    }
}

/// Fills `list` with the sections and returns the section of each row, `None` for the
/// headings. The headings have a button for `add_folder_action` and `add_tag_action`.
pub(crate) fn fill(
    list: &gtk4::ListBox,
    counts: &SectionCounts,
    tags: &[Tag],
    add_folder_action: &str,
    add_tag_action: &str,
) -> Vec<Option<Section>> {
    list.remove_all();
    let mut sections = Vec::new();
    let mut add = |row: gtk4::ListBoxRow, section: Option<Section>| {
        list.append(&row);
        sections.push(section);
    };

    add(
        row(
            icon("folder-documents-symbolic"),
            &gettext("All Notes"),
            counts.all,
        ),
        Some(Section::All),
    );
    add(
        row(
            icon("x-office-document-symbolic"),
            &gettext("Uncategorized"),
            counts.uncategorized,
        ),
        Some(Section::Uncategorized),
    );
    add(
        heading(
            &gettext("Folders"),
            add_folder_action,
            &gettext("New Folder"),
        ),
        None,
    );
    for (folder, count) in &counts.folders {
        add(
            row(icon("folder-symbolic"), folder, *count),
            Some(Section::Folder(folder.clone())),
        );
    }
    add(
        heading(&gettext("Tags"), add_tag_action, &gettext("New Tag")),
        None,
    );
    for tag in tags {
        let count = counts.tags.get(&tag.id).copied().unwrap_or(0);
        add(
            row(tag_dot(tag.color), &tag.name, count),
            Some(Section::Tag(tag.id.clone())),
        );
    }
    add(
        row(icon("user-trash-symbolic"), &gettext("Trash"), counts.trash),
        Some(Section::Trash),
    );
    sections
}

fn icon(name: &str) -> gtk4::Widget {
    gtk4::Image::from_icon_name(name).upcast()
}

/// A dot in the tag's ARGB `color`.
pub(crate) fn tag_dot(color: u32) -> gtk4::Widget {
    let dot = gtk4::DrawingArea::builder()
        .content_width(12)
        .content_height(12)
        .valign(Align::Center)
        .build();
    let [a, r, g, b] = color.to_be_bytes();
    let rgba = gdk::RGBA::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        f32::from(a) / 255.0,
    );
    dot.set_draw_func(move |_, cx, width, height| {
        let radius = f64::from(width.min(height)) / 2.0;
        cx.arc(
            f64::from(width) / 2.0,
            f64::from(height) / 2.0,
            radius,
            0.0,
            std::f64::consts::TAU,
        );
        cx.set_source_rgba(
            f64::from(rgba.red()),
            f64::from(rgba.green()),
            f64::from(rgba.blue()),
            f64::from(rgba.alpha()),
        );
        let _ = cx.fill();
    });
    dot.upcast()
}

fn row(start: gtk4::Widget, label: &str, count: usize) -> gtk4::ListBoxRow {
    let label = gtk4::Label::builder()
        .label(label)
        .xalign(0.0)
        .hexpand(true)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    let count = gtk4::Label::new(Some(&count.to_string()));
    count.add_css_class("dim-label");
    count.add_css_class("numeric");
    let content = gtk4::Box::builder().spacing(12).build();
    content.append(&start);
    content.append(&label);
    content.append(&count);
    gtk4::ListBoxRow::builder().child(&content).build()
}

fn heading(title: &str, add_action: &str, add_tooltip: &str) -> gtk4::ListBoxRow {
    let label = gtk4::Label::builder()
        .label(title)
        .xalign(0.0)
        .hexpand(true)
        .build();
    label.add_css_class("heading");
    label.add_css_class("dim-label");
    let add = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .action_name(add_action)
        .tooltip_text(add_tooltip)
        .build();
    add.add_css_class("flat");
    let content = gtk4::Box::builder().spacing(6).margin_top(12).build();
    content.append(&label);
    content.append(&add);
    gtk4::ListBoxRow::builder()
        .child(&content)
        .activatable(false)
        .selectable(false)
        .build()
}
