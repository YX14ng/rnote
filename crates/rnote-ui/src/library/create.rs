//! Making notebooks in the library: from the "new notebook" dialog, from an image or with
//! the bytes of a notebook made elsewhere (an imported PDF, a copied file).

use std::path::PathBuf;

use gettextrs::gettext;
use gtk4::{glib, subclass::prelude::*};
use rnote_library::creation::{self, NotebookSetup, SetupChoice};
use rnote_library::{Library, Section};
use tracing::error;

use super::{RnLibraryView, error_text, new_notebook};

/// The setup last used in each folder. Outside the library, so it is never synced.
fn defaults_file() -> PathBuf {
    glib::user_data_dir()
        .join("rnote")
        .join("notebook_defaults.json")
}

/// The setup the next notebook of `folder` starts with.
pub(super) fn defaults_for(folder: Option<&str>) -> NotebookSetup {
    creation::defaults_for(&defaults_file(), folder)
}

/// Writes a notebook titled `title` (`untitled` if empty) in `folder` of `library`, with
/// the bytes `make` gives for its file name, all at once and never over another file.
/// Returns its path.
pub(super) fn write_notebook(
    library: &Library,
    folder: Option<&str>,
    title: &str,
    untitled: &str,
    make: impl FnOnce(&str) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<String> {
    let title = title.trim();
    let stem = if title.is_empty() { untitled } else { title };
    let path = library.new_notebook_path(folder, stem)?;
    let file_name = path.rsplit('/').next().unwrap_or(&path);
    let bytes = make(file_name)?;
    rnote_sync::create_notebook(library.root(), &path, &bytes)?;
    Ok(path)
}

impl RnLibraryView {
    /// The folder shown, where new notebooks go; `None` outside a folder.
    pub(super) fn shown_folder(&self) -> Option<String> {
        match self.section() {
            Section::Folder(folder) => Some(folder),
            _ => None,
        }
    }

    /// Asks how the new notebook should be, then makes and opens it.
    pub(super) async fn create_notebook(&self) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let folder = self.shown_folder();
        let folders = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.folders().ok())
            .unwrap_or_default();
        let initial = SetupChoice::from_setup(&defaults_for(folder.as_deref()));
        let Some(chosen) =
            new_notebook::ask_new_notebook(&appwindow, &folders, folder.as_deref(), initial).await
        else {
            return;
        };
        let setup = chosen.choice.setup();
        let folder = chosen.folder.as_deref();
        let made = self.make_notebook(folder, &chosen.title, setup.cover, |file_name| {
            creation::new_notebook_bytes(&setup, file_name)
        });
        if made.is_some()
            && let Err(e) = remember_defaults(folder, &setup)
        {
            error!("Remembering the new notebook's setup failed, Err: {e:?}");
        }
    }

    /// Writes a notebook with the bytes of `make` in `folder`, gives it `cover` and opens
    /// it. Its path, or `None` after telling what went wrong.
    pub(super) fn make_notebook(
        &self,
        folder: Option<&str>,
        title: &str,
        cover: Option<u32>,
        make: impl FnOnce(&str) -> anyhow::Result<Vec<u8>>,
    ) -> Option<String> {
        let written = {
            let library = self.imp().library.borrow();
            let library = library.as_ref()?;
            write_notebook(library, folder, title, &gettext("Untitled"), make)
        };
        let path = match written {
            Ok(path) => path,
            Err(e) => {
                error!("Creating a notebook failed, Err: {e:?}");
                self.toast_error(&error_text::user_message(&e));
                return None;
            }
        };
        // Also shows the library again from disk, with the new notebook, and syncs it.
        self.modify(|library| match cover {
            Some(cover) => library.set_cover(&path, Some(cover)),
            None => Ok(()),
        });
        self.open_path(&path);
        Some(path)
    }

    /// Opens the notebook at `path` in a tab.
    pub(super) fn open_path(&self, path: &str) {
        let Some(appwindow) = self.appwindow() else {
            return;
        };
        let notebook = self
            .imp()
            .library
            .borrow()
            .as_ref()
            .and_then(|library| library.notebooks().ok())
            .and_then(|notebooks| notebooks.into_iter().find(|notebook| notebook.path == path));
        let Some(notebook) = notebook else {
            return;
        };
        let root = self.root();
        glib::spawn_future_local(async move {
            super::open::open_notebook(&appwindow, &root, &notebook).await;
        });
    }
}

fn remember_defaults(folder: Option<&str>, setup: &NotebookSetup) -> anyhow::Result<()> {
    let file = defaults_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    creation::remember_defaults(&file, folder, setup)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rnote_library::Library;

    use super::write_notebook;

    #[test]
    fn writes_the_bytes_under_a_free_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("F")).unwrap();
        fs::write(dir.path().join("F/Álgebra.rnote"), b"old").unwrap();
        let library = Library::open(dir.path()).unwrap();

        let path = write_notebook(&library, Some("F"), "Álgebra", "Untitled", |file_name| {
            Ok(file_name.as_bytes().to_vec())
        })
        .unwrap();

        assert_eq!(path, "F/Álgebra (2).rnote");
        assert_eq!(
            fs::read(dir.path().join(&path)).unwrap(),
            b"\xc3\x81lgebra (2).rnote"
        );
        assert_eq!(
            fs::read(dir.path().join("F/Álgebra.rnote")).unwrap(),
            b"old"
        );
    }

    #[test]
    fn an_empty_title_is_untitled() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();

        let path = write_notebook(&library, None, "  ", "Untitled", |_| Ok(Vec::new())).unwrap();

        assert_eq!(path, "Untitled.rnote");
    }
}
