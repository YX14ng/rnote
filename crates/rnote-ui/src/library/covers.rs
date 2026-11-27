//! The cover submenu of a notebook's card.

use gettextrs::gettext;
use gtk4::{gio, prelude::*};
use rnote_library::COVER_COLORS;

/// The menu target for "no cover", next to the ARGB colors, which are never negative.
const NO_COVER: i64 = -1;

/// The submenu that sets the cover of the notebook at `path`: none, or one of the colors.
pub(super) fn cover_menu(path: &str) -> gio::Menu {
    let menu = gio::Menu::new();
    let item = |label: &str, cover: i64| {
        let item = gio::MenuItem::new(Some(label), None);
        item.set_action_and_target_value(
            Some("library.set-cover"),
            Some(&(path, cover).to_variant()),
        );
        item
    };
    menu.append_item(&item(&gettext("No Cover"), NO_COVER));
    let colors = gio::Menu::new();
    for argb in COVER_COLORS {
        colors.append_item(&item(&cover_name(argb), i64::from(argb)));
    }
    menu.append_section(None, &colors);
    menu
}

/// The cover a menu target stands for.
pub(super) fn cover_from_target(target: i64) -> Option<u32> {
    u32::try_from(target).ok()
}

/// The name of the cover color `argb`.
pub(super) fn cover_name(argb: u32) -> String {
    let names = [
        gettext("Blue"),
        gettext("Green"),
        gettext("Orange"),
        gettext("Red"),
        gettext("Violet"),
        gettext("Turquoise"),
        gettext("Mustard"),
        gettext("Graphite"),
    ];
    COVER_COLORS
        .iter()
        .zip(names)
        .find(|(color, _)| **color == argb)
        .map_or_else(|| format!("#{:06X}", argb & 0xFF_FFFF), |(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::{NO_COVER, cover_from_target};
    use rnote_library::COVER_COLORS;

    #[test]
    fn no_cover_is_none() {
        assert_eq!(cover_from_target(NO_COVER), None);
    }

    #[test]
    fn a_color_target_is_that_color() {
        for argb in COVER_COLORS {
            assert_eq!(cover_from_target(i64::from(argb)), Some(argb));
        }
    }
}
