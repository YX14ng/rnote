//! The window's two views: the library (home screen) and the editor with the tabs.

use adw::prelude::*;
use gtk4::{gio, glib, glib::clone, subclass::prelude::*};

use crate::RnAppWindow;
use crate::library::RnLibraryView;

/// The name of the library page of the window's stack.
const LIBRARY_PAGE: &str = "library";
/// The name of the editor page of the window's stack.
const EDITOR_PAGE: &str = "editor";

impl RnAppWindow {
    /// The library view.
    pub(crate) fn library_view(&self) -> RnLibraryView {
        self.imp().library_view.get()
    }

    /// Reads the library again and shows it. The tabs stay open.
    pub(crate) fn show_library(&self) {
        self.library_view().refresh();
        self.imp().main_stack.set_visible_child_name(LIBRARY_PAGE);
    }

    /// Shows the editor with the tabs.
    pub(crate) fn show_editor(&self) {
        self.imp().main_stack.set_visible_child_name(EDITOR_PAGE);
    }

    /// Sets up the library view, the actions to switch views, and showing the editor
    /// whenever a tab is added.
    pub(super) fn setup_library(&self) {
        self.library_view().init(self);

        let show_library = gio::SimpleAction::new("show-library", None);
        show_library.connect_activate(clone!(
            #[weak(rename_to = appwindow)]
            self,
            move |_, _| appwindow.show_library()
        ));
        self.add_action(&show_library);
        let show_editor = gio::SimpleAction::new("show-editor", None);
        show_editor.connect_activate(clone!(
            #[weak(rename_to = appwindow)]
            self,
            move |_, _| appwindow.show_editor()
        ));
        self.add_action(&show_editor);
        self.app()
            .set_accels_for_action("win.show-library", &["<Alt>Home"]);

        // A new tab, an opened file or a tab moved here is meant to be seen.
        self.overlays().tabview().connect_page_attached(clone!(
            #[weak(rename_to = appwindow)]
            self,
            move |_, _, _| appwindow.show_editor()
        ));
    }
}
