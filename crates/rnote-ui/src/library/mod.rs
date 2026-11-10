//! The library: the home screen of the window, with the notebooks of a folder.

mod card;
mod location;
mod open;
mod sidebar;
mod thumbnails;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use gettextrs::gettext;
use gtk4::{CompositeTemplate, Widget, gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use rnote_library::{Library, Notebook, Section, SortOrder};
use tracing::error;

use crate::RnAppWindow;
use card::Card;
use thumbnails::Thumbnails;

/// The settings key with the library's folder; empty for the default one.
pub(crate) const LIBRARY_DIR_KEY: &str = "library-dir";
/// The settings key (and action) for sorting by name instead of by modification.
const SORT_BY_NAME_KEY: &str = "library-sort-by-name";
/// The settings key (and action) for the list view instead of the grid.
const LIST_VIEW_KEY: &str = "library-list-view";

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
        pub(crate) filling_sidebar: Cell<bool>,
        pub(crate) thumbnails: Thumbnails,
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
                if let Some(section) = section {
                    view.set_section(section);
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
        self.show_notebooks();
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

    /// Fills the sidebar and the cards from the open library.
    fn show_notebooks(&self) {
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
            imp.sidebar_list
                .select_row(imp.sidebar_list.row_at_index(index as i32).as_ref());
        }
        imp.filling_sidebar.set(false);
        *imp.row_sections.borrow_mut() = row_sections;
        *imp.section.borrow_mut() = Some(section.clone());
        imp.title
            .set_title(&sidebar::section_title(&section, &tags));

        let mut shown = rnote_library::filter(&notebooks, &section);
        let order = if self.flag(SORT_BY_NAME_KEY) {
            SortOrder::Name
        } else {
            SortOrder::Modified
        };
        rnote_library::sort(&mut shown, order);
        let list = self.flag(LIST_VIEW_KEY);
        imp.flowbox
            .set_max_children_per_line(if list { 1 } else { 12 });
        imp.flowbox.remove_all();
        let new_card_shown = section != Section::Trash;
        if new_card_shown {
            self.append_card(card::new_card(list), 0);
        }
        let root = library.root().to_owned();
        let first = i32::from(new_card_shown);
        for (index, notebook) in (first..).zip(&shown) {
            self.append_card(
                card::notebook_card(notebook, &root, &imp.thumbnails, list),
                index,
            );
        }
        imp.new_card_shown.set(new_card_shown);
        *imp.shown.borrow_mut() = shown;
    }

    /// Adds `card` as the card at `index`, opening it on click.
    fn append_card(&self, card: Card, index: i32) {
        card.button.connect_clicked(clone!(
            #[weak(rename_to = view)]
            self,
            move |_| view.card_activated(index)
        ));
        let child = gtk4::FlowBoxChild::builder()
            .child(&card.widget)
            .focusable(false)
            .build();
        self.imp().flowbox.append(&child);
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
            open::open_notebook(&appwindow, &root, &notebook).await;
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
        let path = match self
            .imp()
            .library
            .borrow()
            .as_ref()
            .map(|library| library.new_notebook_path(folder.as_deref(), &gettext("Untitled")))
        {
            Some(Ok(path)) => path,
            Some(Err(e)) => {
                error!("Choosing a name for the new notebook failed, Err: {e:?}");
                appwindow
                    .overlays()
                    .dispatch_toast_error(&gettext("Creating the notebook failed"));
                return;
            }
            None => return,
        };
        if let Err(e) = new_notebook_tab(&appwindow, &self.root().join(path)).await {
            error!("Creating the new notebook failed, Err: {e:?}");
            appwindow
                .overlays()
                .dispatch_toast_error(&gettext("Creating the notebook failed"));
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

/// Saves an empty notebook at `file` in the empty draft tab, or a new tab, and shows it.
async fn new_notebook_tab(appwindow: &RnAppWindow, file: &Path) -> anyhow::Result<()> {
    let reusable = appwindow
        .active_tab_wrapper()
        .filter(|wrapper| wrapper.canvas().empty() && wrapper.canvas().output_file().is_none());
    let (wrapper, is_new) = match reusable {
        Some(wrapper) => (wrapper, false),
        None => (appwindow.new_canvas_wrapper(), true),
    };
    wrapper
        .canvas()
        .save_document_to_file(&gio::File::for_path(file))
        .await?;
    if is_new {
        appwindow.append_wrapper_new_tab(&wrapper);
    }
    appwindow.refresh_titles(&wrapper.canvas());
    appwindow.show_editor();
    Ok(())
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
