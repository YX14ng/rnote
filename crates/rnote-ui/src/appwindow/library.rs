//! The window's two views: the library (home screen) and the editor with the tabs.

use std::path::Path;

use adw::prelude::*;
use gtk4::{gio, glib, glib::clone, subclass::prelude::*};

use gettextrs::gettext;
use tracing::error;

use crate::RnAppWindow;
use crate::library::RnLibraryView;
use crate::sync::RnSync;

/// The name of the library page of the window's stack.
const LIBRARY_PAGE: &str = "library";
/// The name of the editor page of the window's stack.
const EDITOR_PAGE: &str = "editor";
/// The window actions that change or use the active document. They are off while the
/// library is shown, so their shortcuts can't touch a document that isn't on screen.
const DOCUMENT_ACTIONS: [&str; 24] = [
    "active-tab-close",
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

    /// The Google Drive sync of the library: one for the whole app, shared by its windows,
    /// so two windows never sync (and lock the library) against each other, and signing out
    /// in one signs out everywhere.
    pub(crate) fn sync(&self) -> RnSync {
        thread_local! {
            static SHARED: std::cell::RefCell<Option<RnSync>> = const { std::cell::RefCell::new(None) };
        }
        self.imp()
            .sync
            .get_or_init(|| {
                SHARED.with(|shared| {
                    shared
                        .borrow_mut()
                        .get_or_insert_with(|| {
                            let sync = RnSync::new(crate::config::APP_ID);
                            // Every window shows the same library; any open one tells where.
                            sync.set_root(|| {
                                gio::Application::default()
                                    .and_downcast::<gtk4::Application>()?
                                    .windows()
                                    .into_iter()
                                    .find_map(|window| window.downcast::<RnAppWindow>().ok())
                                    .map(|window| window.library_view().root())
                            });
                            sync
                        })
                        .clone()
                })
            })
            .clone()
    }

    /// Reads the library again and shows it, and syncs it. The tabs stay open; those of
    /// notebooks of the library are saved first, as Android saves on leaving the editor, so
    /// the sync takes what was just written.
    pub(crate) fn show_library(&self) {
        self.library_view().refresh();
        self.imp().main_stack.set_visible_child_name(LIBRARY_PAGE);
        self.set_document_actions_enabled(false);
        glib::spawn_future_local(clone!(
            #[weak(rename_to = appwindow)]
            self,
            async move {
                appwindow.save_library_tabs().await;
                appwindow.sync().sync_now();
            }
        ));
    }

    /// Saves the tabs of notebooks of the library that have unsaved changes. The library
    /// shows them again by itself once their files change.
    async fn save_library_tabs(&self) {
        let root = self.library_view().root();
        for wrapper in self.get_all_tabs() {
            let canvas = wrapper.canvas();
            let Some(file) = canvas.output_file() else {
                continue;
            };
            if !saves_on_leaving(file.path().as_deref(), canvas.unsaved_changes(), &root) {
                continue;
            }
            if let Err(e) = canvas.save_document_to_file(&file).await {
                error!("Saving a notebook on going to the library failed, Err: {e:?}");
                self.overlays()
                    .dispatch_toast_error(&gettext("Saving document failed"));
            }
        }
    }

    /// Whether the library is on screen, not the editor.
    pub(crate) fn library_shown(&self) -> bool {
        self.imp().main_stack.visible_child_name().as_deref() == Some(LIBRARY_PAGE)
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
        crate::sync::button::setup(&self.library_view().sync_button(), self);
        // Opening the app syncs, once the keyring says it is signed in.
        self.sync().start_up();

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

/// Whether a tab with the file at `path` (`None`: never saved) and `unsaved` changes is saved
/// on going to the library at `root`: only notebooks of the library are.
fn saves_on_leaving(path: Option<&Path>, unsaved: bool, root: &Path) -> bool {
    unsaved && path.is_some_and(|path| path.starts_with(root))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::saves_on_leaving;

    #[test]
    fn unsaved_notebooks_of_the_library_are_saved() {
        let root = Path::new("/notes");

        assert!(saves_on_leaving(
            Some(Path::new("/notes/Álgebra/Parcial.rnote")),
            true,
            root
        ));
        assert!(!saves_on_leaving(
            Some(Path::new("/notes/Álgebra/Parcial.rnote")),
            false,
            root
        ));
    }

    #[test]
    fn other_files_and_new_documents_are_left_alone() {
        let root = Path::new("/notes");

        assert!(!saves_on_leaving(
            Some(Path::new("/home/me/suelto.rnote")),
            true,
            root
        ));
        assert!(!saves_on_leaving(
            Some(Path::new("/notes-old/x.rnote")),
            true,
            root
        ));
        assert!(!saves_on_leaving(None, true, root));
    }
}
