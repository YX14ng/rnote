//! The window's two views: the library (home screen) and the editor with the tabs.

use adw::prelude::*;
use gtk4::{gio, glib, glib::clone, subclass::prelude::*};

use crate::RnAppWindow;
use crate::library::RnLibraryView;

/// The name of the library page of the window's stack.
const LIBRARY_PAGE: &str = "library";
/// The name of the editor page of the window's stack.
const EDITOR_PAGE: &str = "editor";
/// The window actions that change or use the active document. They are off while the
/// library is shown, so their shortcuts can't touch a document that isn't on screen.
const DOCUMENT_ACTIONS: [&str; 23] = [
    "undo",
    "redo",
    "clear-doc",
    "add-page-to-doc",
    "remove-page-from-doc",
    "import-file",
    "print-doc",
    "clipboard-copy",
    "clipboard-cut",
    "clipboard-paste",
    "text-bold",
    "text-italic",
    "text-underline",
    "pen-style",
    "set-color-1",
    "set-color-2",
    "set-color-3",
    "set-color-4",
    "set-color-5",
    "set-color-6",
    "set-color-7",
    "set-color-8",
    "set-color-9",
];

impl RnAppWindow {
    /// The library view.
    pub(crate) fn library_view(&self) -> RnLibraryView {
        self.imp().library_view.get()
    }

    /// Reads the library again and shows it. The tabs stay open.
    pub(crate) fn show_library(&self) {
        self.library_view().refresh();
        self.imp().main_stack.set_visible_child_name(LIBRARY_PAGE);
        self.set_document_actions_enabled(false);
    }

    /// Shows the editor with the tabs.
    pub(crate) fn show_editor(&self) {
        self.imp().main_stack.set_visible_child_name(EDITOR_PAGE);
        self.set_document_actions_enabled(true);
    }

    fn set_document_actions_enabled(&self, enabled: bool) {
        for name in DOCUMENT_ACTIONS {
            if let Some(action) = self.lookup_action(name)
                && let Ok(action) = action.downcast::<gio::SimpleAction>()
            {
                action.set_enabled(enabled);
            }
        }
        // Enabling an action makes its buttons sensitive again, but the undo and redo
        // buttons follow the document's history instead.
        if enabled && let Some(canvas) = self.active_tab_canvas() {
            let (can_undo, can_redo) = {
                let engine = canvas.engine_ref();
                (engine.can_undo(), engine.can_redo())
            };
            let penpicker = self.overlays().penpicker();
            penpicker.undo_button().set_sensitive(can_undo);
            penpicker.redo_button().set_sensitive(can_redo);
        }
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
