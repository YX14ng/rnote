//! The library: the home screen of the window, with the notebooks of a folder.

mod actions;
mod busy;
mod card;
mod dialogs;
mod grid;
mod location;
mod manage;
mod open;
mod sidebar;
mod tabs;
mod tagchoice;
mod thumbnails;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use gettextrs::gettext;
use gtk4::{CompositeTemplate, Widget, gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use rnote_library::{Library, Notebook, Section};
use tracing::error;

use crate::RnAppWindow;
use thumbnails::Thumbnails;

/// The settings key with the library's folder; empty for the default one.
pub(crate) const LIBRARY_DIR_KEY: &str = "library-dir";
/// The settings key (and action) for sorting by name instead of by modification.
const SORT_BY_NAME_KEY: &str = "library-sort-by-name";
/// The settings key (and action) for the list view instead of the grid.
const LIST_VIEW_KEY: &str = "library-list-view";
/// The action for selection mode.
const SELECTION_MODE: &str = "selection-mode";

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/flxzt/rnote/ui/library/libraryview.ui")]
    pub(crate) struct RnLibraryView {
        pub(crate) appwindow: glib::WeakRef<RnAppWindow>,
        pub(crate) library: RefCell<Option<Library>>,
        pub(crate) section: RefCell<Option<Section>>,
        /// The section of each sidebar row, `None` for headings.
        pub(crate) row_sections: RefCell<Vec<Option<Section>>>,
        /// The notebooks shown, in the order of their cards (after "New").
        pub(crate) shown: RefCell<Vec<Notebook>>,
        pub(crate) new_card_shown: Cell<bool>,
        /// The paths of the notebooks selected in selection mode.
        pub(crate) selected: RefCell<BTreeSet<String>>,
        pub(crate) filling_sidebar: Cell<bool>,
        pub(crate) thumbnails: Thumbnails,
        pub(crate) busy: busy::Busy,
        pub(crate) actions: gio::SimpleActionGroup,

        #[template_child]
        pub(crate) title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(crate) stack: TemplateChild<gtk4::Stack>,
        #[template_child]
        pub(crate) split: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub(crate) sidebar_list: TemplateChild<gtk4::ListBox>,
        #[template_child]
        pub(crate) flowbox: TemplateChild<gtk4::FlowBox>,
        #[template_child]
        pub(crate) error_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub(crate) section_menu_button: TemplateChild<gtk4::MenuButton>,
        #[template_child]
        pub(crate) select_button: TemplateChild<gtk4::ToggleButton>,
        #[template_child]
        pub(crate) empty_trash_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub(crate) selection_bar: TemplateChild<gtk4::ActionBar>,
        #[template_child]
        pub(crate) selection_label: TemplateChild<gtk4::Label>,
        #[template_child]
        pub(crate) selection_move_button: TemplateChild<gtk4::MenuButton>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RnLibraryView {
        const NAME: &'static str = "RnLibraryView";
        type Type = super::RnLibraryView;
        type ParentType = Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for RnLibraryView {
        fn constructed(&self) {
            self.parent_constructed();
            // Before anything looks up its actions: GTK doesn't point the action lookup of
            // widgets made earlier at a group inserted later.
            self.obj().setup_actions();
        }

        fn dispose(&self) {
            self.dispose_template();
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for RnLibraryView {}
}

glib::wrapper! {
    pub(crate) struct RnLibraryView(ObjectSubclass<imp::RnLibraryView>)
        @extends Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl Default for RnLibraryView {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl RnLibraryView {
    /// Connects the view to its window and loads its saved view options.
    pub(crate) fn init(&self, appwindow: &RnAppWindow) {
        let imp = self.imp();
        imp.appwindow.set(Some(appwindow));
        if let Some(settings) = appwindow.app().app_settings() {
            for key in [SORT_BY_NAME_KEY, LIST_VIEW_KEY] {
                imp.actions
                    .change_action_state(key, &settings.boolean(key).to_variant());
            }
        }

        imp.sidebar_list.connect_row_selected(clone!(
            #[weak(rename_to = view)]
            self,
            move |_, row| {
                if view.imp().filling_sidebar.get() {
                    return;
                }
                let Some(row) = row else { return };
                let section = usize::try_from(row.index())
                    .ok()
                    .and_then(|index| view.imp().row_sections.borrow().get(index).cloned())
                    .flatten();
                // The sidebar is filled again for the section: not while it is still
                // handling the selection of the row it would drop.
                if let Some(section) = section {
                    glib::idle_add_local_once(clone!(
                        #[weak]
                        view,
                        move || view.set_section(section)
                    ));
                }
            }
        ));
    }

    fn setup_actions(&self) {
        let actions = &self.imp().actions;
        let choose_folder = gio::SimpleAction::new("choose-folder", None);
        choose_folder.connect_activate(clone!(
            #[weak(rename_to = view)]
            self,
            move |_, _| {
                glib::spawn_future_local(clone!(
                    #[weak]
                    view,
                    async move { view.choose_folder().await }
                ));
            }
        ));
        actions.add_action(&choose_folder);
        let refresh = gio::SimpleAction::new("refresh", None);
        refresh.connect_activate(clone!(
            #[weak(rename_to = view)]
            self,
            move |_, _| view.refresh()
        ));
        actions.add_action(&refresh);

        // View options, saved to the settings when the window is known.
        for key in [SORT_BY_NAME_KEY, LIST_VIEW_KEY] {
            let action = gio::SimpleAction::new_stateful(key, None, &false.to_variant());
            action.connect_activate(clone!(
                #[weak(rename_to = view)]
                self,
                move |action, _| {
                    let value = !action
                        .state()
                        .and_then(|state| state.get::<bool>())
                        .unwrap_or(false);
                    action.set_state(&value.to_variant());
                    let settings = view
                        .appwindow()
                        .and_then(|appwindow| appwindow.app().app_settings());
                    if let Some(settings) = settings
                        && let Err(e) = settings.set_boolean(&action.name(), value)
                    {
                        error!("Saving {} failed, Err: {e:?}", action.name());
                    }
                    view.show_notebooks();
                }
            ));
            actions.add_action(&action);
        }
        let selection_mode =
            gio::SimpleAction::new_stateful(SELECTION_MODE, None, &false.to_variant());
        selection_mode.connect_activate(clone!(
            #[weak(rename_to = view)]
            self,
            move |action, _| {
                let value = !action
                    .state()
                    .and_then(|state| state.get::<bool>())
                    .unwrap_or(false);
                view.set_selection_mode(value);
            }
        ));
        actions.add_action(&selection_mode);

        self.add_notebook_actions(actions);
        self.add_manage_actions(actions);
        self.insert_action_group("library", Some(actions));
    }

    fn appwindow(&self) -> Option<RnAppWindow> {
        self.imp().appwindow.upgrade()
    }

    /// The state of the boolean action `name`.
    fn flag(&self, name: &str) -> bool {
        self.imp()
            .actions
            .action_state(name)
            .and_then(|state| state.get::<bool>())
            .unwrap_or(false)
    }

    /// Turns selection mode on or off, starting with nothing selected.
    fn set_selection_mode(&self, on: bool) {
        let imp = self.imp();
        imp.actions
            .change_action_state(SELECTION_MODE, &on.to_variant());
        imp.selected.borrow_mut().clear();
        self.show_notebooks();
    }

    /// Shows `message` as an error toast in the window.
    fn toast_error(&self, message: &str) {
        if let Some(appwindow) = self.appwindow() {
            appwindow.overlays().dispatch_toast_error(message);
        }
    }

    /// Runs `op` on the open library, shows its error as a toast, then shows the library
    /// again from disk. `None` when it failed or no library is open.
    fn modify<T>(&self, op: impl FnOnce(&mut Library) -> anyhow::Result<T>) -> Option<T> {
        let result = {
            let mut library = self.imp().library.borrow_mut();
            library.as_mut().map(op)
        };
        let value = match result {
            Some(Ok(value)) => Some(value),
            Some(Err(e)) => {
                error!("A library operation failed, Err: {e:?}");
                self.toast_error(&format!("{e:#}"));
                None
            }
            None => None,
        };
        self.refresh();
        value
    }

    /// The `library-dir` setting, empty when unset or without settings.
    fn root_setting(&self) -> String {
        self.appwindow()
            .and_then(|appwindow| appwindow.app().app_settings())
            .map(|settings| settings.string(LIBRARY_DIR_KEY).to_string())
            .unwrap_or_default()
    }

    /// The library's folder.
    pub(crate) fn root(&self) -> PathBuf {
        location::library_root(
            &self.root_setting(),
            glib::user_special_dir(glib::UserDirectory::Documents),
            &glib::home_dir(),
        )
    }

    /// The section shown.
    pub(crate) fn section(&self) -> Section {
        self.imp().section.borrow().clone().unwrap_or(Section::All)
    }

    /// Shows the notebooks of `section`.
    pub(crate) fn set_section(&self, section: Section) {
        if self.imp().section.borrow().as_ref() == Some(&section) {
            return;
        }
        *self.imp().section.borrow_mut() = Some(section);
        self.set_selection_mode(false);
    }

    /// Opens the library again from disk and shows it, or shows why it can't be opened.
    pub(crate) fn refresh(&self) {
        let imp = self.imp();
        let root = self.root();
        imp.title.set_subtitle(&root.to_string_lossy());
        match open_library(&root, self.root_setting().is_empty()) {
            Ok(library) => {
                *imp.library.borrow_mut() = Some(library);
                imp.stack.set_visible_child_name("content");
                self.show_notebooks();
            }
            Err(e) => {
                error!(
                    "Opening the library at {} failed, Err: {e:?}",
                    root.display()
                );
                *imp.library.borrow_mut() = None;
                imp.error_page
                    .set_description(Some(&glib::markup_escape_text(&format!(
                        "{}\n{e:#}",
                        root.display()
                    ))));
                imp.stack.set_visible_child_name("error");
            }
        }
    }

    /// Lets the user pick the library's folder, then shows it.
    async fn choose_folder(&self) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let dialog = gtk4::FileDialog::builder()
            .title(gettext("Library Folder"))
            .accept_label(gettext("Choose"))
            .modal(true)
            .build();
        let root = self.root();
        if root.is_dir() {
            dialog.set_initial_folder(Some(&gio::File::for_path(&root)));
        }
        let Ok(folder) = dialog.select_folder_future(Some(&appwindow)).await else {
            return;
        };
        let Some(path) = folder.path() else { return };
        match appwindow.app().app_settings() {
            Some(settings) => {
                if let Err(e) = settings.set_string(LIBRARY_DIR_KEY, &path.to_string_lossy()) {
                    error!("Saving the library folder failed, Err: {e:?}");
                }
            }
            None => {
                appwindow.overlays().dispatch_toast_error(&gettext(
                    "Settings schema is not installed. App settings could not be loaded and won't be saved.",
                ));
            }
        }
        *self.imp().section.borrow_mut() = None;
        self.refresh();
    }
}

/// Opens the library at `root`. Only the default folder is created when missing: a set
/// one that is missing may be an unmounted disk, which must not get an empty library.
fn open_library(root: &Path, is_default: bool) -> anyhow::Result<Library> {
    if is_default {
        std::fs::create_dir_all(root).with_context(|| format!("Creating {}", root.display()))?;
    }
    if !root.is_dir() {
        bail!("{}", gettext("The folder doesn’t exist"));
    }
    Library::open(root)
}
