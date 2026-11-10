//! The library: the home screen of the window, with the notebooks of a folder.

mod location;

use std::cell::RefCell;
use std::path::PathBuf;

use anyhow::{Context, bail};
use gettextrs::gettext;
use gtk4::{CompositeTemplate, Widget, gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use rnote_library::Library;
use tracing::error;

use crate::RnAppWindow;

/// The settings key with the library's folder; empty for the default one.
pub(crate) const LIBRARY_DIR_KEY: &str = "library-dir";

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/flxzt/rnote/ui/library/libraryview.ui")]
    pub(crate) struct RnLibraryView {
        pub(crate) appwindow: glib::WeakRef<RnAppWindow>,
        pub(crate) library: RefCell<Option<Library>>,

        #[template_child]
        pub(crate) title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(crate) stack: TemplateChild<gtk4::Stack>,
        #[template_child]
        pub(crate) empty_page: TemplateChild<adw::StatusPage>,
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
    /// Connects the view to its window and sets up its actions.
    pub(crate) fn init(&self, appwindow: &RnAppWindow) {
        self.imp().appwindow.set(Some(appwindow));

        let actions = gio::SimpleActionGroup::new();
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
        self.insert_action_group("library", Some(&actions));
    }

    fn appwindow(&self) -> Option<RnAppWindow> {
        self.imp().appwindow.upgrade()
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

    /// Opens the library again from disk and shows it, or shows why it can't be opened.
    pub(crate) fn refresh(&self) {
        let imp = self.imp();
        let root = self.root();
        imp.title.set_subtitle(&root.to_string_lossy());
        match open_library(&root, self.root_setting().is_empty()) {
            Ok(library) => {
                *imp.library.borrow_mut() = Some(library);
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

    fn show_notebooks(&self) {
        let imp = self.imp();
        let count = imp
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.notebooks().ok())
            .map_or(0, |notebooks| notebooks.len());
        imp.empty_page.set_description(Some(&count.to_string()));
        imp.stack.set_visible_child_name("content");
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
        self.refresh();
    }
}

/// Opens the library at `root`. Only the default folder is created when missing: a set
/// one that is missing may be an unmounted disk, which must not get an empty library.
fn open_library(root: &std::path::Path, is_default: bool) -> anyhow::Result<Library> {
    if is_default {
        std::fs::create_dir_all(root).with_context(|| format!("Creating {}", root.display()))?;
    }
    if !root.is_dir() {
        bail!("{}", gettext("The folder doesn’t exist"));
    }
    Library::open(root)
}
