//! Managing the folders and tags of the library.

use gettextrs::gettext;
use gtk4::{gio, subclass::prelude::*};
use rnote_library::Section;

use super::{RnLibraryView, actions, dialogs, tabs};

/// The colors offered to new tags, in turn (ARGB, GNOME palette).
const TAG_COLORS: [u32; 6] = [
    0xFF35_84E4,
    0xFF33_D17A,
    0xFFF6_D32D,
    0xFFFF_7800,
    0xFFE0_1B24,
    0xFF91_41AC,
];

impl RnLibraryView {
    /// Adds the actions on folders and tags to `actions`.
    pub(super) fn add_manage_actions(&self, actions: &gio::SimpleActionGroup) {
        self.add_async(actions, "new-folder", None, |view, _| async move {
            view.new_folder().await
        });
        self.add_async(actions, "rename-folder", None, |view, _| async move {
            view.rename_folder().await;
        });
        self.add_async(actions, "delete-folder", None, |view, _| async move {
            view.delete_folder().await;
        });
        self.add_async(actions, "new-tag", None, |view, _| async move {
            view.new_tag().await
        });
        self.add_async(actions, "edit-tag", None, |view, _| async move {
            view.edit_tag().await
        });
        self.add_async(actions, "delete-tag", None, |view, _| async move {
            view.delete_tag().await;
        });
    }

    async fn new_folder(&self) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let Some(name) =
            dialogs::ask_name(&appwindow, &gettext("New Folder"), "", &gettext("Create")).await
        else {
            return;
        };
        if let Some(folder) = self.modify(|library| library.create_folder(&name)) {
            self.set_section(Section::Folder(folder));
        }
    }

    async fn rename_folder(&self) {
        let Section::Folder(folder) = self.section() else {
            return;
        };
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let Some(name) = dialogs::ask_name(
            &appwindow,
            &gettext("Rename Folder"),
            &folder,
            &gettext("Rename"),
        )
        .await
        else {
            return;
        };
        let root = self.root();
        if let Some(new_folder) = self.modify(|library| library.rename_folder(&folder, &name)) {
            tabs::retarget_open_tabs(&appwindow, &root.join(&folder), &root.join(&new_folder));
            self.set_section(Section::Folder(new_folder));
        }
    }

    /// Deletes the folder shown, sending what it has to the trash. Asks first when it has
    /// something, and refuses while a notebook in it is open.
    async fn delete_folder(&self) {
        let Section::Folder(folder) = self.section() else {
            return;
        };
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let dir = self.root().join(&folder);
        if let Some(name) = tabs::open_tab_inside(&appwindow, &dir) {
            self.toast_error(&actions::close_first(&name));
            return;
        }
        let has_files = std::fs::read_dir(&dir).is_ok_and(|mut entries| entries.next().is_some());
        if has_files {
            let heading = gettext("Delete “{folder}”?").replace("{folder}", &folder);
            let body = gettext("Its notebooks go to the trash.");
            if !dialogs::confirm(&appwindow, &heading, &body, &gettext("Delete")).await {
                return;
            }
        }
        if self
            .modify(|library| library.delete_folder(&folder))
            .is_some()
        {
            self.set_section(Section::All);
        }
    }

    async fn new_tag(&self) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let count = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map_or(0, |library| library.tags().len());
        let color = TAG_COLORS[count % TAG_COLORS.len()];
        let Some((name, color)) = dialogs::ask_tag(
            &appwindow,
            &gettext("New Tag"),
            "",
            color,
            &gettext("Create"),
        )
        .await
        else {
            return;
        };
        if let Some(id) = self.modify(|library| library.create_tag(&name, color)) {
            self.set_section(Section::Tag(id));
        }
    }

    async fn edit_tag(&self) {
        let Section::Tag(id) = self.section() else {
            return;
        };
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let tag = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.tags().into_iter().find(|tag| tag.id == id));
        let Some(tag) = tag else { return };
        let Some((name, color)) = dialogs::ask_tag(
            &appwindow,
            &gettext("Edit Tag"),
            &tag.name,
            tag.color,
            &gettext("Save"),
        )
        .await
        else {
            return;
        };
        self.modify(|library| library.update_tag(&id, &name, color));
    }

    async fn delete_tag(&self) {
        let Section::Tag(id) = self.section() else {
            return;
        };
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let heading = gettext("Delete Tag?");
        let body = gettext("The notebooks stay; only the tag is removed from them.");
        if !dialogs::confirm(&appwindow, &heading, &body, &gettext("Delete")).await {
            return;
        }
        if self.modify(|library| library.delete_tag(&id)).is_some() {
            self.set_section(Section::All);
        }
    }
}
