//! Notebooks of the library open in the editor: where they are in the library, and what
//! to do when a sync changes them under the editor.

use std::path::{Component, Path, PathBuf};

use gettextrs::gettext;
use gtk4::{gio, glib, glib::clone, prelude::*};
use rnote_sync::WrittenNotebook;
use tracing::error;

use crate::{RnAppWindow, RnCanvas};

/// Where a save goes in the library: the library's folder and the notebook's path in it.
pub(crate) type LibraryTarget = (PathBuf, String);

/// The library target of saving `canvas` to `file`, if `file` is a notebook of the library.
pub(crate) fn library_target(canvas: &RnCanvas, file: &Path) -> Option<LibraryTarget> {
    let appwindow = canvas.root().and_downcast::<RnAppWindow>()?;
    let root = appwindow.library_view().root();
    let path = library_path(&root, file)?;
    Some((root, path))
}

/// Writes `bytes` to the library notebook `target`, unless it changed since the editor
/// read the version with hash `expected`: then they go to a conflict copy beside it.
pub(crate) async fn write(
    target: &LibraryTarget,
    expected: Option<String>,
    bytes: Vec<u8>,
) -> anyhow::Result<WrittenNotebook> {
    let (root, path) = target.clone();
    let device = glib::host_name().to_string();
    gio::spawn_blocking(move || {
        let now = chrono::Local::now().naive_local();
        rnote_sync::write_notebook(&root, &path, expected.as_deref(), &device, now, &bytes)
    })
    .await
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Saving stopped unexpectedly")))
}

/// After `canvas` saved `written` in the library at `root`: follows a conflict copy, tells
/// about it, and syncs soon.
pub(crate) fn saved(canvas: &RnCanvas, root: &Path, written: &WrittenNotebook) {
    canvas.set_library_sha256(Some(written.sha256.clone()));
    let Some(appwindow) = canvas.root().and_downcast::<RnAppWindow>() else {
        return;
    };
    if written.conflict {
        canvas.set_output_file(Some(gio::File::for_path(root.join(&written.path))));
        // The file the editor had wasn't written, so no event of it will clear this.
        canvas.set_output_file_expect_write(false);
        let name = written.path.rsplit('/').next().unwrap_or(&written.path);
        appwindow.overlays().dispatch_toast_text(
            &gettext("It changed on another device meanwhile: saved as “{name}”")
                .replace("{name}", name),
            None,
        );
    }
    appwindow.sync().sync_soon();
}

/// Reloads `canvas` when its library notebook changed on disk and it has nothing unsaved;
/// returns whether it did, else the caller offers to reload as before.
pub(crate) fn reload_if_clean(appwindow: &RnAppWindow, canvas: &RnCanvas) -> bool {
    let in_library = canvas
        .output_file()
        .and_then(|file| file.path())
        .is_some_and(|path| library_path(&appwindow.library_view().root(), &path).is_some());
    if on_changed_on_disk(canvas.unsaved_changes(), in_library) != Reaction::Reload {
        return false;
    }
    glib::spawn_future_local(clone!(
        #[weak]
        appwindow,
        #[weak]
        canvas,
        async move {
            if let Err(e) = canvas.reload_from_disk().await {
                error!("Reloading a notebook changed on another device failed: {e:?}");
                appwindow
                    .overlays()
                    .dispatch_toast_error(&gettext("Reloading .rnote file from disk failed"));
            } else {
                canvas.emit_invalidate_thumbnail();
                appwindow.overlays().dispatch_toast_text(
                    &gettext("Updated with the changes from another device"),
                    None,
                );
            }
        }
    ));
    true
}

/// The path of `file` in the library at `root`, with `/`, if it is a notebook the sync
/// keeps: a `.rnote` inside the root and outside hidden folders.
pub(crate) fn library_path(root: &Path, file: &Path) -> Option<String> {
    let relative = file.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return None;
        };
        let name = name.to_str()?;
        if name.starts_with('.') {
            return None;
        }
        parts.push(name);
    }
    let path = parts.join("/");
    path.ends_with(".rnote").then_some(path)
}

/// What the editor does when its file changed on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reaction {
    /// Reload it, with a notice: nothing in the editor would be lost.
    Reload,
    /// Offer to reload, as Rnote always did.
    AskAsBefore,
}

/// The reaction for a file with `unsaved` changes, `in_library` or not. Saving a library
/// notebook with unsaved changes later puts them in a conflict copy, so offering is safe.
pub(crate) fn on_changed_on_disk(unsaved: bool, in_library: bool) -> Reaction {
    if in_library && !unsaved {
        Reaction::Reload
    } else {
        Reaction::AskAsBefore
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Reaction, library_path, on_changed_on_disk};

    #[test]
    fn library_path_inside() {
        let root = Path::new("/mnt/Storage/HandNotes");

        assert_eq!(
            library_path(root, Path::new("/mnt/Storage/HandNotes/Algebra/a.rnote")).as_deref(),
            Some("Algebra/a.rnote")
        );
        assert_eq!(
            library_path(root, &root.join("a.rnote")).as_deref(),
            Some("a.rnote")
        );
    }

    #[test]
    fn library_path_is_none_outside_the_library() {
        let root = Path::new("/mnt/Storage/HandNotes");

        for path in [
            "/home/me/Documents/a.rnote",
            "/mnt/Storage/HandNotes2/a.rnote",
            "/mnt/Storage/HandNotes/a.xopp",
            "/mnt/Storage/HandNotes/.rnote-library/trash/a.rnote",
            "/mnt/Storage/HandNotes/../a.rnote",
        ] {
            assert_eq!(library_path(root, Path::new(path)), None, "{path}");
        }
    }

    #[test]
    fn changed_on_disk_reloads_only_clean_library_notebooks() {
        assert_eq!(on_changed_on_disk(false, true), Reaction::Reload);
        assert_eq!(on_changed_on_disk(true, true), Reaction::AskAsBefore);
        assert_eq!(on_changed_on_disk(false, false), Reaction::AskAsBefore);
        assert_eq!(on_changed_on_disk(true, false), Reaction::AskAsBefore);
    }
}
