//! The small dialogs of the library: a name, a confirmation, a tag, tags to check.

use std::collections::BTreeSet;

use adw::prelude::*;
use gettextrs::gettext;
use gtk4::{gdk, glib, glib::clone};
use rnote_library::Tag;

use crate::RnAppWindow;

/// The response that accepts a dialog.
const ACCEPT: &str = "accept";
/// The response that cancels a dialog.
const CANCEL: &str = "cancel";

/// An alert dialog with "Cancel" and `accept`, the default.
fn alert(heading: &str, body: &str, accept: &str, destructive: bool) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::builder()
        .heading(heading)
        .body(body)
        .default_response(ACCEPT)
        .close_response(CANCEL)
        .build();
    dialog.add_responses(&[(CANCEL, &gettext("Cancel")), (ACCEPT, accept)]);
    dialog.set_response_appearance(
        ACCEPT,
        if destructive {
            adw::ResponseAppearance::Destructive
        } else {
            adw::ResponseAppearance::Suggested
        },
    );
    dialog
}

/// Asks for a name, starting from `initial`. `None` when cancelled or left empty.
pub(crate) async fn ask_name(
    appwindow: &RnAppWindow,
    heading: &str,
    initial: &str,
    accept: &str,
) -> Option<String> {
    let dialog = alert(heading, "", accept, false);
    let entry = gtk4::Entry::builder()
        .text(initial)
        .activates_default(true)
        .build();
    dialog.set_extra_child(Some(&entry));
    entry.connect_changed(clone!(
        #[weak]
        dialog,
        move |entry| dialog.set_response_enabled(ACCEPT, !entry.text().trim().is_empty())
    ));
    dialog.set_response_enabled(ACCEPT, !initial.trim().is_empty());
    dialog.set_focus(Some(&entry));
    entry.select_region(0, -1);
    if dialog.choose_future(Some(appwindow)).await != ACCEPT {
        return None;
    }
    Some(entry.text().trim().to_owned()).filter(|name| !name.is_empty())
}

/// Asks to confirm `heading`. True when confirmed.
pub(crate) async fn confirm(
    appwindow: &RnAppWindow,
    heading: &str,
    body: &str,
    accept: &str,
) -> bool {
    alert(heading, body, accept, true)
        .choose_future(Some(appwindow))
        .await
        == ACCEPT
}

/// Asks for a tag's name and color, starting from `name` and the ARGB `color`.
pub(crate) async fn ask_tag(
    appwindow: &RnAppWindow,
    heading: &str,
    name: &str,
    color: u32,
    accept: &str,
) -> Option<(String, u32)> {
    let dialog = alert(heading, "", accept, false);
    let entry = gtk4::Entry::builder()
        .text(name)
        .activates_default(true)
        .hexpand(true)
        .build();
    let color_button = gtk4::ColorDialogButton::builder()
        .dialog(&gtk4::ColorDialog::builder().with_alpha(false).build())
        .rgba(&argb_to_rgba(color))
        .tooltip_text(gettext("Color"))
        .build();
    let content = gtk4::Box::builder().spacing(6).build();
    content.append(&entry);
    content.append(&color_button);
    dialog.set_extra_child(Some(&content));
    entry.connect_changed(clone!(
        #[weak]
        dialog,
        move |entry| dialog.set_response_enabled(ACCEPT, !entry.text().trim().is_empty())
    ));
    dialog.set_response_enabled(ACCEPT, !name.trim().is_empty());
    dialog.set_focus(Some(&entry));
    if dialog.choose_future(Some(appwindow)).await != ACCEPT {
        return None;
    }
    let name = entry.text().trim().to_owned();
    (!name.is_empty()).then(|| (name, rgba_to_argb(&color_button.rgba())))
}

/// Asks which of `tags` to check, with `checked` checked at first. `None` when cancelled.
pub(crate) async fn choose_tags(
    appwindow: &RnAppWindow,
    tags: &[Tag],
    checked: &BTreeSet<String>,
) -> Option<BTreeSet<String>> {
    let dialog = alert(&gettext("Tags"), "", &gettext("Apply"), false);
    let list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .build();
    list.add_css_class("boxed-list");
    let mut checks = Vec::new();
    for tag in tags {
        let check = gtk4::CheckButton::builder()
            .active(checked.contains(&tag.id))
            .build();
        let row = adw::ActionRow::builder()
            .title(&tag.name)
            .activatable_widget(&check)
            .build();
        row.add_prefix(&check);
        row.add_suffix(&super::sidebar::tag_dot(tag.color));
        list.append(&row);
        checks.push((tag.id.clone(), check));
    }
    if tags.is_empty() {
        dialog.set_body(&gettext(
            "There are no tags yet: create one in the sidebar.",
        ));
        dialog.set_response_enabled(ACCEPT, false);
    } else {
        let scrolled = gtk4::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(360)
            .build();
        dialog.set_extra_child(Some(&scrolled));
    }
    if dialog.choose_future(Some(appwindow)).await != ACCEPT {
        return None;
    }
    Some(
        checks
            .into_iter()
            .filter(|(_, check)| check.is_active())
            .map(|(id, _)| id)
            .collect(),
    )
}

/// An ARGB color as a GDK color.
pub(crate) fn argb_to_rgba(color: u32) -> gdk::RGBA {
    let [a, r, g, b] = color.to_be_bytes();
    gdk::RGBA::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        f32::from(a) / 255.0,
    )
}

/// A GDK color as ARGB, opaque.
fn rgba_to_argb(color: &gdk::RGBA) -> u32 {
    // The channels are in 0..=1, so the rounded values fit a byte.
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    u32::from_be_bytes([
        0xFF,
        byte(color.red()),
        byte(color.green()),
        byte(color.blue()),
    ])
}
