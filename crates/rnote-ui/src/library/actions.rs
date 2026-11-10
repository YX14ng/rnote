//! The actions on notebooks: the card menus, the trash and the selection.

use std::collections::BTreeSet;
use std::path::Path;

use gettextrs::gettext;
use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use rnote_library::{Notebook, Section, TrashEntry};

use super::{RnLibraryView, dialogs, tabs, tagchoice};

/// The menu of a notebook's card.
pub(super) fn notebook_menu(notebook: &Notebook, folders: &[String]) -> gio::Menu {
    let path = notebook.path.to_variant();
    let menu = gio::Menu::new();
    let edit = gio::Menu::new();
    edit.append_item(&item(&gettext("Rename…"), "library.rename", &path));
    edit.append_item(&item(&gettext("Tags…"), "library.tags", &path));
    edit.append_item(&item(&gettext("Duplicate"), "library.duplicate", &path));
    menu.append_section(None, &edit);
    let moves = gio::Menu::new();
    moves.append_submenu(
        Some(&gettext("Move to")),
        &move_menu("library.move", Some(notebook), folders),
    );
    moves.append_item(&item(&gettext("Move to Trash"), "library.trash", &path));
    menu.append_section(None, &moves);
    menu
}

/// The menu of a card in the trash.
pub(super) fn trash_menu(entry: &TrashEntry) -> gio::Menu {
    let id = entry.id.to_variant();
    let menu = gio::Menu::new();
    menu.append_item(&item(&gettext("Restore"), "library.restore", &id));
    menu.append_item(&item(
        &gettext("Delete Forever…"),
        "library.delete-forever",
        &id,
    ));
    menu
}

/// The places to move to: "Uncategorized" and the folders, without the one `notebook` is
/// in. `action` takes the folder (`""` for none), after the notebook's path if given.
pub(super) fn move_menu(
    action: &str,
    notebook: Option<&Notebook>,
    folders: &[String],
) -> gio::Menu {
    let menu = gio::Menu::new();
    let current = notebook.map(|notebook| notebook.folder.clone().unwrap_or_default());
    let target = |folder: &str| match notebook {
        Some(notebook) => (notebook.path.as_str(), folder).to_variant(),
        None => folder.to_variant(),
    };
    if current.as_deref() != Some("") {
        menu.append_item(&item(&gettext("Uncategorized"), action, &target("")));
    }
    for folder in folders
        .iter()
        .filter(|folder| current.as_deref() != Some(folder.as_str()))
    {
        menu.append_item(&item(folder, action, &target(folder)));
    }
    menu
}

/// The menu of the section shown, for a folder or a tag.
pub(super) fn section_menu(section: &Section) -> Option<gio::Menu> {
    let menu = gio::Menu::new();
    match section {
        Section::Folder(_) => {
            menu.append(
                Some(&gettext("Rename Folder…")),
                Some("library.rename-folder"),
            );
            menu.append(
                Some(&gettext("Delete Folder…")),
                Some("library.delete-folder"),
            );
        }
        Section::Tag(_) => {
            menu.append(Some(&gettext("Edit Tag…")), Some("library.edit-tag"));
            menu.append(Some(&gettext("Delete Tag…")), Some("library.delete-tag"));
        }
        _ => return None,
    }
    Some(menu)
}

fn item(label: &str, action: &str, target: &glib::Variant) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some(action), Some(target));
    item
}

impl RnLibraryView {
    /// Adds the actions on notebooks to `actions`.
    pub(super) fn add_notebook_actions(&self, actions: &gio::SimpleActionGroup) {
        let string = Some(glib::VariantTy::STRING);
        let path_and_folder = Some(glib::VariantTy::new("(ss)").expect("a valid type"));
        self.add_async(actions, "rename", string, |view, arg| async move {
            if let Some(path) = arg.and_then(|arg| arg.get::<String>()) {
                view.rename(&path).await;
            }
        });
        self.add_async(actions, "move", path_and_folder, |view, arg| async move {
            if let Some((path, folder)) = arg.and_then(|arg| arg.get::<(String, String)>()) {
                view.move_notebooks(&[path], &folder);
            }
        });
        self.add_async(actions, "duplicate", string, |view, arg| async move {
            if let Some(path) = arg.and_then(|arg| arg.get::<String>()) {
                view.modify(|library| library.duplicate(&path));
            }
        });
        self.add_async(actions, "tags", string, |view, arg| async move {
            if let Some(path) = arg.and_then(|arg| arg.get::<String>()) {
                view.choose_tags(vec![path]).await;
            }
        });
        self.add_async(actions, "trash", string, |view, arg| async move {
            if let Some(path) = arg.and_then(|arg| arg.get::<String>()) {
                view.trash(&[path]);
            }
        });
        self.add_async(actions, "restore", string, |view, arg| async move {
            if let Some(id) = arg.and_then(|arg| arg.get::<String>()) {
                view.modify(|library| library.restore(&id));
            }
        });
        self.add_async(actions, "delete-forever", string, |view, arg| async move {
            let Some(id) = arg.and_then(|arg| arg.get::<String>()) else {
                return;
            };
            let Some(appwindow) = view.appwindow() else {
                return;
            };
            let heading = gettext("Delete Forever?");
            let body = gettext("The notebook can’t be recovered.");
            if dialogs::confirm(&appwindow, &heading, &body, &gettext("Delete")).await {
                view.modify(|library| library.delete_forever(&id));
            }
        });
        self.add_async(actions, "empty-trash", None, |view, _| async move {
            let Some(appwindow) = view.appwindow() else {
                return;
            };
            let heading = gettext("Empty Trash?");
            let body = gettext("The notebooks in the trash can’t be recovered.");
            if dialogs::confirm(&appwindow, &heading, &body, &gettext("Empty Trash")).await {
                view.modify(|library| library.empty_trash());
            }
        });
        self.add_async(actions, "selection-move", string, |view, arg| async move {
            if let Some(folder) = arg.and_then(|arg| arg.get::<String>()) {
                let paths = view.selected_paths();
                view.move_notebooks(&paths, &folder);
                view.set_selection_mode(false);
            }
        });
        self.add_async(actions, "selection-tags", None, |view, _| async move {
            let paths = view.selected_paths();
            view.choose_tags(paths).await;
            view.set_selection_mode(false);
        });
        self.add_async(actions, "selection-trash", None, |view, _| async move {
            let paths = view.selected_paths();
            view.trash(&paths);
            view.set_selection_mode(false);
        });
    }

    /// Adds the action `name` to `actions`, running `run` on the main loop.
    pub(super) fn add_async<F, Fut>(
        &self,
        actions: &gio::SimpleActionGroup,
        name: &str,
        parameter: Option<&glib::VariantTy>,
        run: F,
    ) where
        F: Fn(RnLibraryView, Option<glib::Variant>) -> Fut + Clone + 'static,
        Fut: std::future::Future<Output = ()> + 'static,
    {
        let action = gio::SimpleAction::new(name, parameter);
        action.connect_activate(clone!(
            #[weak(rename_to = view)]
            self,
            move |_, arg| {
                glib::spawn_future_local(run(view.clone(), arg.cloned()));
            }
        ));
        actions.add_action(&action);
    }

    fn selected_paths(&self) -> Vec<String> {
        self.imp().selected.borrow().iter().cloned().collect()
    }

    /// Asks for a new name for the notebook at `path` and renames it, with its tabs.
    async fn rename(&self, path: &str) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let stem = Path::new(path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned());
        let Some(name) = dialogs::ask_name(
            &appwindow,
            &gettext("Rename Notebook"),
            &stem.unwrap_or_default(),
            &gettext("Rename"),
        )
        .await
        else {
            return;
        };
        let root = self.root();
        if let Some(new_path) = self.modify(|library| library.rename(path, &name)) {
            tabs::retarget_open_tabs(&appwindow, &root.join(path), &root.join(new_path));
        }
    }

    /// Moves the notebooks at `paths` to `folder` (`""` for none), with their tabs.
    fn move_notebooks(&self, paths: &[String], folder: &str) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let root = self.root();
        let folder = Some(folder).filter(|folder| !folder.is_empty());
        for path in paths {
            if let Some(new_path) = self.modify(|library| library.move_to(path, folder)) {
                tabs::retarget_open_tabs(&appwindow, &root.join(path), &root.join(new_path));
            }
        }
    }

    /// Sends the notebooks at `paths` to the trash, except the ones open in a tab.
    fn trash(&self, paths: &[String]) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let root = self.root();
        for path in paths {
            if let Some(name) = tabs::open_tab_inside(&appwindow, &root.join(path)) {
                self.toast_error(&close_first(&name));
                continue;
            }
            self.modify(|library| library.trash(path));
        }
    }

    /// Lets the user choose the tags of the notebooks at `paths`.
    async fn choose_tags(&self, paths: Vec<String>) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let (tags, current) = {
            let library = self.imp().library.borrow();
            let Some(library) = library.as_ref() else {
                return;
            };
            let notebooks = library.notebooks().unwrap_or_default();
            let current: Vec<(String, Vec<String>)> = paths
                .iter()
                .filter_map(|path| notebooks.iter().find(|notebook| &notebook.path == path))
                .map(|notebook| (notebook.path.clone(), notebook.tags.clone()))
                .collect();
            (library.tags(), current)
        };
        let all_tags: Vec<Vec<String>> = current.iter().map(|(_, tags)| tags.clone()).collect();
        let initially = tagchoice::tags_all_have(&all_tags);
        let Some(checked) = dialogs::choose_tags(&appwindow, &tags, &initially).await else {
            return;
        };
        let changes: BTreeSet<_> = checked.symmetric_difference(&initially).collect();
        if changes.is_empty() {
            return;
        }
        for (path, tags) in current {
            let new_tags = tagchoice::apply_tag_choice(&tags, &initially, &checked);
            if new_tags != tags {
                self.modify(|library| library.set_tags(&path, &new_tags));
            }
        }
    }
}

/// "Close “{name}” first", for a notebook open in a tab.
pub(super) fn close_first(name: &str) -> String {
    gettext("Close “{name}” first").replace("{name}", name)
}
