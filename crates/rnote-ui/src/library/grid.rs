//! Filling the library view: the sidebar, the header for the section and the cards.

use gettextrs::{gettext, ngettext};
use gtk4::{glib, glib::clone, prelude::*, subclass::prelude::*};
use rnote_library::{Library, Notebook, Section, SortOrder};
use tracing::error;

use super::card::{self, Card};
use super::{LIST_VIEW_KEY, RnLibraryView, SELECTION_MODE, SORT_BY_NAME_KEY, actions, sidebar};

impl RnLibraryView {
    /// Fills the sidebar, the header and the cards from the open library.
    pub(super) fn show_notebooks(&self) {
        let imp = self.imp();
        let library = imp.library.borrow();
        let Some(library) = library.as_ref() else {
            return;
        };
        let notebooks = library.notebooks().unwrap_or_else(|e| {
            error!("Listing the notebooks failed, Err: {e:?}");
            Vec::new()
        });
        let folders = library.folders().unwrap_or_default();
        let tags = library.tags();
        let counts = rnote_library::counts(&notebooks, &folders, library.trash_entries().len());

        imp.filling_sidebar.set(true);
        let row_sections = sidebar::fill(
            &imp.sidebar_list,
            &counts,
            &tags,
            "library.new-folder",
            "library.new-tag",
        );
        let mut section = self.section();
        if !row_sections.contains(&Some(section.clone())) {
            section = Section::All;
        }
        if let Some(index) = row_sections
            .iter()
            .position(|row| row.as_ref() == Some(&section))
        {
            let index = i32::try_from(index).unwrap_or_default();
            imp.sidebar_list
                .select_row(imp.sidebar_list.row_at_index(index).as_ref());
        }
        imp.filling_sidebar.set(false);
        *imp.row_sections.borrow_mut() = row_sections;
        *imp.section.borrow_mut() = Some(section.clone());
        imp.title
            .set_title(&sidebar::section_title(&section, &tags));

        let selecting = self.flag(SELECTION_MODE) && section != Section::Trash;
        self.show_header_for(&section, &folders, selecting);
        let list = self.flag(LIST_VIEW_KEY);
        imp.flowbox
            .set_max_children_per_line(if list { 1 } else { 12 });
        imp.flowbox.remove_all();
        if section == Section::Trash {
            self.show_trash(library, list);
            imp.new_card_shown.set(false);
            imp.shown.borrow_mut().clear();
            return;
        }

        let mut shown = rnote_library::filter(&notebooks, &section);
        let order = if self.flag(SORT_BY_NAME_KEY) {
            SortOrder::Name
        } else {
            SortOrder::Modified
        };
        rnote_library::sort(&mut shown, order);
        let new_card_shown = !selecting;
        if new_card_shown {
            self.append_card(&card::new_card(list), 0);
        }
        let root = library.root().to_owned();
        let first = i32::from(new_card_shown);
        for (index, notebook) in (first..).zip(&shown) {
            let card = card::notebook_card(notebook, &root, &imp.thumbnails, list);
            if selecting {
                self.make_selectable(&card, notebook);
                self.append_child(&card);
            } else {
                card.set_menu(actions::notebook_menu(notebook, &folders).upcast_ref());
                self.append_card(&card, index);
            }
        }
        imp.new_card_shown.set(new_card_shown);
        *imp.shown.borrow_mut() = shown;
        self.update_selection_label();
    }

    /// Shows the header buttons and the selection bar that fit `section`.
    fn show_header_for(&self, section: &Section, folders: &[String], selecting: bool) {
        let imp = self.imp();
        let is_trash = *section == Section::Trash;
        imp.select_button.set_visible(!is_trash);
        imp.empty_trash_button.set_visible(is_trash);
        match actions::section_menu(section) {
            Some(menu) => {
                imp.section_menu_button.set_menu_model(Some(&menu));
                imp.section_menu_button.set_visible(true);
            }
            None => imp.section_menu_button.set_visible(false),
        }
        imp.selection_bar.set_revealed(selecting);
        imp.selection_move_button
            .set_menu_model(Some(&actions::move_menu(
                "library.selection-move",
                None,
                folders,
            )));
    }

    /// The cards of the trash, the most recently deleted first; a click opens their menu.
    fn show_trash(&self, library: &Library, list: bool) {
        let mut entries = library.trash_entries();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.deleted_at_ms));
        for entry in &entries {
            let card = card::trash_card(entry, list);
            card.set_menu(actions::trash_menu(entry).upcast_ref());
            let menu_button = card.menu_button.clone();
            card.button.connect_clicked(move |_| menu_button.popup());
            self.append_child(&card);
        }
    }

    fn append_child(&self, card: &Card) {
        let child = gtk4::FlowBoxChild::builder()
            .child(&card.widget)
            .focusable(false)
            .build();
        self.imp().flowbox.append(&child);
    }

    /// Adds `card` as the card at `index`, opening it on click.
    fn append_card(&self, card: &Card, index: i32) {
        card.button.connect_clicked(clone!(
            #[weak(rename_to = view)]
            self,
            move |_| view.card_activated(index)
        ));
        self.append_child(card);
    }

    /// Makes `card` toggle the selection of `notebook` instead of opening it.
    fn make_selectable(&self, card: &Card, notebook: &Notebook) {
        let path = notebook.path.clone();
        card.set_selectable(self.imp().selected.borrow().contains(&path));
        card.check.connect_toggled(clone!(
            #[weak(rename_to = view)]
            self,
            move |check| {
                let mut selected = view.imp().selected.borrow_mut();
                if check.is_active() {
                    selected.insert(path.clone());
                } else {
                    selected.remove(&path);
                }
                drop(selected);
                view.update_selection_label();
            }
        ));
        let check = card.check.clone();
        card.button
            .connect_clicked(move |_| check.set_active(!check.is_active()));
    }

    fn update_selection_label(&self) {
        let count = self.imp().selected.borrow().len();
        let count_u32 = u32::try_from(count).unwrap_or(u32::MAX);
        self.imp().selection_label.set_label(
            &ngettext("{n} selected", "{n} selected", count_u32).replace("{n}", &count.to_string()),
        );
        for name in ["selection-move", "selection-tags", "selection-trash"] {
            if let Some(action) = self.imp().actions.lookup_action(name)
                && let Ok(action) = action.downcast::<gtk4::gio::SimpleAction>()
            {
                action.set_enabled(count > 0);
            }
        }
    }

    /// Opens the notebook of the card at `index`, or creates one for "New".
    fn card_activated(&self, index: i32) {
        let imp = self.imp();
        let Ok(mut index) = usize::try_from(index) else {
            return;
        };
        if imp.new_card_shown.get() {
            if index == 0 {
                glib::spawn_future_local(clone!(
                    #[weak(rename_to = view)]
                    self,
                    async move { view.create_notebook().await }
                ));
                return;
            }
            index -= 1;
        }
        let Some(notebook) = imp.shown.borrow().get(index).cloned() else {
            return;
        };
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let root = self.root();
        glib::spawn_future_local(async move {
            super::open::open_notebook(&appwindow, &root, &notebook).await;
        });
    }

    /// Creates "Untitled.rnote" in the folder shown (the root elsewhere) and opens it.
    async fn create_notebook(&self) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let folder = match self.section() {
            Section::Folder(folder) => Some(folder),
            _ => None,
        };
        let path = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.new_notebook_path(folder.as_deref(), &gettext("Untitled")));
        let path = match path {
            Some(Ok(path)) => path,
            Some(Err(e)) => {
                error!("Choosing a name for the new notebook failed, Err: {e:?}");
                self.toast_error(&gettext("Creating the notebook failed"));
                return;
            }
            None => return,
        };
        if let Err(e) = super::open::new_notebook_tab(&appwindow, &self.root().join(path)).await {
            error!("Creating the new notebook failed, Err: {e:?}");
            self.toast_error(&gettext("Creating the notebook failed"));
        }
    }
}
