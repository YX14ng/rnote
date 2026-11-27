//! Searching the library by name, and what an empty screen says.

use gettextrs::gettext;
use gtk4::{glib, prelude::*, subclass::prelude::*};
use rnote_library::Section;

use super::RnLibraryView;

/// What the screen says when it shows no notebook while searching `query` (empty when not
/// searching) in `section`; `None` where the "New" card is there anyway.
pub(super) fn empty_message(section: &Section, query: &str) -> Option<String> {
    let query = query.trim();
    if !query.is_empty() {
        return Some(gettext("No notebook is named like “{name}”").replace("{name}", query));
    }
    match section {
        Section::Trash => Some(gettext("The trash is empty")),
        Section::Folder(_) => Some(gettext("This folder has no notebooks yet")),
        Section::Tag(_) => Some(gettext("No notebook has this tag yet")),
        Section::All | Section::Uncategorized => None,
    }
}

impl RnLibraryView {
    /// The search typed, trimmed; empty when the search bar is closed.
    pub(super) fn query(&self) -> String {
        let imp = self.imp();
        if !imp.search_bar.is_search_mode() {
            return String::new();
        }
        imp.search_entry.text().trim().to_owned()
    }

    /// Searches as the user types, also when typing starts anywhere in the library.
    pub(super) fn setup_search(&self) {
        let imp = self.imp();
        imp.search_bar.connect_entry(&*imp.search_entry);
        imp.search_bar.set_key_capture_widget(Some(self));
        imp.search_entry.connect_search_changed(glib::clone!(
            #[weak(rename_to = view)]
            self,
            move |_| view.show_notebooks()
        ));
        imp.search_bar
            .connect_search_mode_enabled_notify(glib::clone!(
                #[weak(rename_to = view)]
                self,
                move |_| view.show_notebooks()
            ));
    }

    /// Shows `message` over the empty grid, or hides it.
    pub(super) fn show_empty(&self, message: Option<String>) {
        let label = &self.imp().empty_label;
        label.set_label(message.as_deref().unwrap_or_default());
        label.set_visible(message.is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_screen_says_why() {
        assert_eq!(
            empty_message(&Section::Folder("Orga".to_owned()), " xyz "),
            Some("No notebook is named like “xyz”".to_owned())
        );
        assert_eq!(
            empty_message(&Section::Trash, ""),
            Some("The trash is empty".to_owned())
        );
        assert_eq!(
            empty_message(&Section::Folder("Orga".to_owned()), ""),
            Some("This folder has no notebooks yet".to_owned())
        );
        assert_eq!(
            empty_message(&Section::Tag("t".to_owned()), ""),
            Some("No notebook has this tag yet".to_owned())
        );
        assert_eq!(empty_message(&Section::All, ""), None);
    }
}
