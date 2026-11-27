//! Bringing files into the library: notebooks (.rnote, .xopp) copied as they are, and PDFs
//! and images made into new notebooks. From the Import menu or dropped on the library.

use std::path::Path;

use gettextrs::gettext;
use gtk4::{gdk, gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use rnote_library::creation;
use tracing::error;

use super::{RnLibraryView, create, error_text};

/// What a file becomes in the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImportKind {
    /// A notebook, copied as it is.
    Notebook,
    /// A PDF, made into a notebook with its pages under the ink.
    Pdf,
    /// An image, made into a notebook with it on the first page.
    Image,
}

const NOTEBOOK_EXTENSIONS: [&str; 2] = ["rnote", "xopp"];
/// The image formats the engine can read.
const IMAGE_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff"];

/// What the file named `file_name` becomes, by its extension; `None` if it can't be
/// imported.
pub(super) fn import_kind(file_name: &str) -> Option<ImportKind> {
    let extension = Path::new(file_name)
        .extension()?
        .to_string_lossy()
        .to_lowercase();
    if NOTEBOOK_EXTENSIONS.contains(&extension.as_str()) {
        Some(ImportKind::Notebook)
    } else if extension == "pdf" {
        Some(ImportKind::Pdf)
    } else if IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        Some(ImportKind::Image)
    } else {
        None
    }
}

/// The file name of `file`.
fn file_name(file: &gio::File) -> String {
    file.basename()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The name of `file_name` without its extension.
fn stem(file_name: &str) -> String {
    Path::new(file_name).file_stem().map_or_else(
        || file_name.to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

/// The whole contents of `file`.
pub(super) async fn load(file: &gio::File) -> anyhow::Result<Vec<u8>> {
    let (bytes, _) = file.load_contents_future().await?;
    Ok(bytes.to_vec())
}

fn filter(name: &str, extensions: &[&str]) -> gtk4::FileFilter {
    let filter = gtk4::FileFilter::new();
    filter.set_name(Some(name));
    for extension in extensions {
        filter.add_suffix(extension);
    }
    filter
}

impl RnLibraryView {
    /// Adds the actions of the Import menu to `actions`.
    pub(super) fn add_import_actions(&self, actions: &gio::SimpleActionGroup) {
        self.add_async(actions, "import-notebooks", None, |view, _| async move {
            let filter = filter(&gettext("Notebooks"), &NOTEBOOK_EXTENSIONS);
            view.pick_and_import(&filter, true).await;
        });
        self.add_async(actions, "import-pdf", None, |view, _| async move {
            view.pick_and_import(&filter(&gettext("PDF"), &["pdf"]), false)
                .await;
        });
        self.add_async(actions, "import-image", None, |view, _| async move {
            view.pick_and_import(&filter(&gettext("Images"), &IMAGE_EXTENSIONS), false)
                .await;
        });
    }

    /// Lets files dropped on the library be imported into the folder shown.
    pub(super) fn setup_drop(&self) {
        let target = gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        target.connect_drop(clone!(
            #[weak(rename_to = view)]
            self,
            #[upgrade_or]
            false,
            move |_, value, _, _| {
                let Ok(list) = value.get::<gdk::FileList>() else {
                    return false;
                };
                let files = list.files();
                glib::spawn_future_local(async move { view.import_files(files).await });
                true
            }
        ));
        self.add_controller(target);
    }

    /// Lets the user pick files of `filter` (several if `multiple`) and imports them.
    async fn pick_and_import(&self, filter: &gtk4::FileFilter, multiple: bool) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let filters = gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(filter);
        let dialog = gtk4::FileDialog::builder()
            .title(gettext("Import"))
            .modal(true)
            .filters(&filters)
            .default_filter(filter)
            .build();
        let files = if multiple {
            match dialog.open_multiple_future(Some(&appwindow)).await {
                Ok(list) => list.iter::<gio::File>().filter_map(Result::ok).collect(),
                Err(_) => return,
            }
        } else {
            match dialog.open_future(Some(&appwindow)).await {
                Ok(file) => vec![file],
                Err(_) => return,
            }
        };
        self.import_files(files).await;
    }

    /// Imports `files` into the folder shown, each as its kind says; tells which ones
    /// can't be imported.
    pub(super) async fn import_files(&self, files: Vec<gio::File>) {
        let folder = self.shown_folder();
        for file in files {
            let name = file_name(&file);
            match import_kind(&name) {
                Some(ImportKind::Notebook) => {
                    self.copy_notebook(&file, folder.as_deref(), &name).await
                }
                Some(ImportKind::Pdf) => {
                    self.import_pdf(&file, folder.as_deref(), &stem(&name))
                        .await
                }
                Some(ImportKind::Image) => {
                    self.import_image(&file, folder.as_deref(), &stem(&name))
                        .await
                }
                None => self.toast_error(
                    &gettext("“{name}” can’t be imported: only notebooks, PDFs and images can")
                        .replace("{name}", &name),
                ),
            }
        }
    }

    /// Copies the notebook `file`, named `name`, into `folder`, under a free name.
    async fn copy_notebook(&self, file: &gio::File, folder: Option<&str>, name: &str) {
        let copied = async {
            let bytes = load(file).await?;
            let path = {
                let library = self.imp().library.borrow();
                let library = library
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("No library is open"))?;
                library.free_path(folder, name)?
            };
            rnote_sync::create_notebook(&self.root(), &path, &bytes)?;
            anyhow::Ok(())
        }
        .await;
        if let Err(e) = copied {
            error!("Importing {name} failed, Err: {e:?}");
            self.toast_error(&error_text::user_message(&e));
        }
        self.changed();
    }

    /// Makes a notebook titled `title` in `folder` with the image `file` on its first page,
    /// set up as the last one there, and opens it.
    async fn import_image(&self, file: &gio::File, folder: Option<&str>, title: &str) {
        let bytes = match load(file).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Reading the image failed, Err: {e:?}");
                self.toast_error(&error_text::user_message(&e));
                return;
            }
        };
        let setup = create::defaults_for(folder);
        self.make_notebook(folder, title, setup.cover, move |file_name| {
            creation::image_notebook_bytes(&setup, &bytes, file_name)
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::{ImportKind, import_kind};

    #[test]
    fn files_are_told_apart_by_their_extension() {
        assert_eq!(import_kind("Álgebra.rnote"), Some(ImportKind::Notebook));
        assert_eq!(import_kind("Orga.XOPP"), Some(ImportKind::Notebook));
        assert_eq!(import_kind("guía.pdf"), Some(ImportKind::Pdf));
        assert_eq!(import_kind("pizarrón.JPG"), Some(ImportKind::Image));
        assert_eq!(import_kind("foto.jpeg"), Some(ImportKind::Image));
        assert_eq!(import_kind("captura.png"), Some(ImportKind::Image));
        assert_eq!(import_kind("notas.txt"), None);
        assert_eq!(import_kind("sin extensión"), None);
    }
}
