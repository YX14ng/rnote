//! Opening a notebook of the library.

use std::path::{Path, PathBuf};

use adw::prelude::*;
use gettextrs::gettext;
use gtk4::gio;
use rnote_library::{Library, Notebook, NotebookKind};
use tracing::error;

use crate::RnAppWindow;

/// What opening a notebook does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpenPlan {
    /// Open the `.rnote` at this relative path.
    Rnote(String),
    /// A Xournal++ notebook imported before, as the `.rnote` at `existing`: ask whether to
    /// open that one or import again.
    AskReimport { existing: String },
    /// A Xournal++ notebook never imported: import it, then open the result.
    Import,
}

/// What opening `notebook` of `library` does.
pub(crate) fn open_plan(library: &Library, notebook: &Notebook) -> OpenPlan {
    match notebook.kind {
        NotebookKind::Rnote => OpenPlan::Rnote(notebook.path.clone()),
        NotebookKind::Xopp => match library.imported_rnote_of(&notebook.path) {
            Some(existing) => OpenPlan::AskReimport { existing },
            None => OpenPlan::Import,
        },
    }
}

/// Opens `notebook` of the library at `root` in a tab, or focuses the tab that has it. A
/// Xournal++ notebook is imported first, asking when it was imported before.
pub(crate) async fn open_notebook(appwindow: &RnAppWindow, root: &Path, notebook: &Notebook) {
    let plan = match Library::open(root) {
        Ok(library) => open_plan(&library, notebook),
        Err(e) => {
            error!(
                "Opening the library at {} failed, Err: {e:?}",
                root.display()
            );
            appwindow
                .overlays()
                .dispatch_toast_error(&gettext("Opening file failed"));
            return;
        }
    };
    let path = match plan {
        OpenPlan::Rnote(path) => path,
        OpenPlan::AskReimport { existing } => {
            match ask_reimport(appwindow, notebook, &existing).await {
                Some(Reimport::Open) => existing,
                Some(Reimport::Import) => match import(appwindow, root, &notebook.path).await {
                    Some(path) => path,
                    None => return,
                },
                None => return,
            }
        }
        OpenPlan::Import => match import(appwindow, root, &notebook.path).await {
            Some(path) => path,
            None => return,
        },
    };
    appwindow
        .open_file_w_dialogs(gio::File::for_path(root.join(path)), None, true)
        .await;
}

/// The answer to "already imported".
enum Reimport {
    /// Open the earlier import.
    Open,
    /// Import again, into a new notebook.
    Import,
}

/// Asks whether to open `existing`, the earlier import of `notebook`, or to import it
/// again. `None` when cancelled.
async fn ask_reimport(
    appwindow: &RnAppWindow,
    notebook: &Notebook,
    existing: &str,
) -> Option<Reimport> {
    let existing_name = Path::new(existing).file_name().map_or_else(
        || existing.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Already Imported"))
        .body(
            gettext("“{name}” was imported before as “{existing}”.")
                .replace("{name}", &notebook.name)
                .replace("{existing}", &existing_name),
        )
        .default_response("open")
        .close_response("cancel")
        .build();
    dialog.add_responses(&[
        ("cancel", &gettext("Cancel")),
        ("import", &gettext("Import Again")),
        ("open", &gettext("Open")),
    ]);
    dialog.set_response_appearance("open", adw::ResponseAppearance::Suggested);
    match dialog.choose_future(Some(appwindow)).await.as_str() {
        "open" => Some(Reimport::Open),
        "import" => Some(Reimport::Import),
        _ => None,
    }
}

/// Imports the Xournal++ notebook at `path` of the library at `root` off the main thread.
/// The relative path of the new notebook, or `None` after showing the error.
async fn import(appwindow: &RnAppWindow, root: &Path, path: &str) -> Option<String> {
    let overlays = appwindow.overlays();
    overlays.progressbar_start_pulsing();
    let root_buf: PathBuf = root.to_owned();
    let path_owned = path.to_owned();
    let result =
        gio::spawn_blocking(move || Library::open(&root_buf)?.import_xopp(&path_owned)).await;
    match result {
        Ok(Ok(new_path)) => {
            overlays.progressbar_finish();
            appwindow.library_view().refresh();
            Some(new_path)
        }
        Ok(Err(e)) => {
            error!("Importing {path} failed, Err: {e:?}");
            overlays.progressbar_abort();
            overlays.dispatch_toast_error(&gettext("Importing the Xournal++ notebook failed"));
            None
        }
        Err(_) => {
            error!("Importing {path} panicked");
            overlays.progressbar_abort();
            overlays.dispatch_toast_error(&gettext("Importing the Xournal++ notebook failed"));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rnote_library::Library;

    use super::{OpenPlan, open_plan};

    fn notebook_of(library: &Library, path: &str) -> rnote_library::Notebook {
        library
            .notebooks()
            .unwrap()
            .into_iter()
            .find(|n| n.path == path)
            .unwrap()
    }

    #[test]
    fn rnote_opens_directly() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("F")).unwrap();
        fs::write(dir.path().join("F/a.rnote"), b"a").unwrap();
        let library = Library::open(dir.path()).unwrap();

        let plan = open_plan(&library, &notebook_of(&library, "F/a.rnote"));

        assert_eq!(plan, OpenPlan::Rnote("F/a.rnote".to_owned()));
    }

    #[test]
    fn imported_xopp_asks_before_importing_again() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Orga.xopp"), b"x").unwrap();
        fs::write(dir.path().join("Orga.rnote"), b"r").unwrap();
        let library = Library::open(dir.path()).unwrap();

        let plan = open_plan(&library, &notebook_of(&library, "Orga.xopp"));

        assert_eq!(
            plan,
            OpenPlan::AskReimport {
                existing: "Orga.rnote".to_owned()
            }
        );
    }

    #[test]
    fn new_xopp_imports() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Orga.xopp"), b"x").unwrap();
        let library = Library::open(dir.path()).unwrap();

        assert_eq!(
            open_plan(&library, &notebook_of(&library, "Orga.xopp")),
            OpenPlan::Import
        );
    }
}
