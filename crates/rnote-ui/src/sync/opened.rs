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

/// Which version of a notebook a save may replace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Expected {
    /// The editor's own file: only the version it read or last wrote, with this hash (none
    /// for a file it never read). Another version goes to a conflict copy.
    Read(Option<String>),
    /// Another file the user chose to replace (e.g. "Save As"): whatever is there now.
    WhateverIsThere,
}

impl Expected {
    /// For saving to the editor's own file or not, with the hash of what it read.
    pub(crate) fn for_save(own_file: bool, read: Option<String>) -> Self {
        if own_file {
            Self::Read(read)
        } else {
            Self::WhateverIsThere
        }
    }
}

/// Writes `bytes` to the library notebook `target`, unless it isn't the version `expected`
/// anymore: then they go to a conflict copy beside it.
pub(crate) async fn write(
    target: &LibraryTarget,
    expected: Expected,
    bytes: Vec<u8>,
) -> anyhow::Result<WrittenNotebook> {
    let (root, path) = target.clone();
    let device = glib::host_name().to_string();
    gio::spawn_blocking(move || {
        let now = chrono::Local::now().naive_local();
        write_blocking(&root, &path, expected, &device, now, &bytes)
    })
    .await
    .unwrap_or_else(|_| Err(anyhow::anyhow!("Saving stopped unexpectedly")))
}

fn write_blocking(
    root: &Path,
    path: &str,
    expected: Expected,
    device: &str,
    now: chrono::NaiveDateTime,
    bytes: &[u8],
) -> anyhow::Result<WrittenNotebook> {
    let expected = match expected {
        Expected::Read(sha256) => sha256,
        Expected::WhateverIsThere => std::fs::read(root.join(path))
            .ok()
            .map(|current| rnote_sync::sha256_hex(&current)),
    };
    rnote_sync::write_notebook(root, path, expected.as_deref(), device, now, bytes)
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

/// Reads the file `canvas` shows, without showing it yet: its snapshot, hash and path.
async fn read_snapshot(
    canvas: &RnCanvas,
) -> anyhow::Result<(
    rnote_engine::engine::EngineSnapshot,
    Option<String>,
    PathBuf,
)> {
    let file = canvas
        .output_file()
        .ok_or_else(|| anyhow::anyhow!("No file to reload"))?;
    let path = file
        .path()
        .ok_or_else(|| anyhow::anyhow!("The file has no path"))?;
    let (bytes, _) = file.load_bytes_future().await?;
    let to_hash = bytes.clone();
    // Off the main thread, like the reading itself.
    let sha256 = Some(
        gio::spawn_blocking(move || rnote_sync::sha256_hex(&to_hash))
            .await
            .map_err(|_| anyhow::anyhow!("Hashing the file stopped unexpectedly"))?,
    );
    let snapshot =
        rnote_engine::engine::EngineSnapshot::load_from_rnote_bytes(bytes.to_vec()).await?;
    Ok((snapshot, sha256, path))
}

/// Whether `file`, open in a tab, is one of the notebooks of the library at `root` that a
/// sync just wrote (`changed`, relative paths with `/`).
pub(crate) fn changed_by_sync(root: &Path, file: &Path, changed: &[String]) -> bool {
    library_path(root, file).is_some_and(|path| changed.contains(&path))
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
            match read_snapshot(&canvas).await {
                Err(e) => {
                    error!("Reloading a notebook changed on another device failed: {e:?}");
                    appwindow
                        .overlays()
                        .dispatch_toast_error(&gettext("Reloading .rnote file from disk failed"));
                }
                // Written on meanwhile: nothing is replaced. Saving keeps both versions, since
                // the file isn't the one the editor read anymore.
                Ok(_) if canvas.unsaved_changes() => {
                    appwindow.overlays().dispatch_toast_text(
                        &gettext("It changed on another device: saving keeps both versions"),
                        None,
                    );
                }
                // Already what the tab shows, e.g. told both by the sync and the file watcher.
                Ok((_, sha256, _)) if sha256.is_some() && sha256 == canvas.library_sha256() => {}
                Ok((snapshot, sha256, path)) => {
                    // The file also holds the view of the device that wrote it: this one's
                    // stays, as on Android.
                    let (offset, zoom) = {
                        let engine = canvas.engine_ref();
                        (engine.camera.offset(), engine.camera.zoom())
                    };
                    let mut widget_flags =
                        canvas.apply_rnote_snapshot(snapshot, sha256, Some(path));
                    {
                        let mut engine = canvas.engine_mut();
                        widget_flags |= engine.camera.zoom_to(zoom);
                        widget_flags |= engine.camera_set_offset_expand(offset);
                    }
                    canvas.emit_handle_widget_flags(widget_flags);
                    canvas.emit_invalidate_thumbnail();
                    appwindow.overlays().dispatch_toast_text(
                        &gettext("Updated with the changes from another device"),
                        None,
                    );
                }
            }
        }
    ));
    true
}

/// The path of `file` in the library at `root`, with `/`, if it is a notebook the sync
/// keeps: a `.rnote` inside the root and outside hidden folders. A file reached through a
/// link (or a library set to one) counts by where it really is.
pub(crate) fn library_path(root: &Path, file: &Path) -> Option<String> {
    notebook_path(root, file).or_else(|| {
        let root = root.canonicalize().ok()?;
        // The folder only: the notebook itself being a link to elsewhere doesn't count.
        let folder = file.parent()?.canonicalize().ok()?;
        notebook_path(&root, &folder.join(file.file_name()?))
    })
}

fn notebook_path(root: &Path, file: &Path) -> Option<String> {
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

    use super::{
        Expected, Reaction, changed_by_sync, library_path, on_changed_on_disk, write_blocking,
    };

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
    fn library_path_through_a_link_to_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("HandNotes");
        std::fs::create_dir_all(root.join("Algebra")).unwrap();
        std::fs::write(root.join("Algebra/a.rnote"), b"").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();

        // The file opened through the link, and the library set to the link.
        assert_eq!(
            library_path(&root, &link.join("Algebra/a.rnote")).as_deref(),
            Some("Algebra/a.rnote")
        );
        assert_eq!(
            library_path(&link, &root.join("Algebra/a.rnote")).as_deref(),
            Some("Algebra/a.rnote")
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

    fn at() -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 12)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap()
    }

    #[test]
    fn save_as_over_an_existing_notebook_replaces_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rnote"), b"theirs").unwrap();

        let written = write_blocking(
            dir.path(),
            "a.rnote",
            Expected::WhateverIsThere,
            "PC",
            at(),
            b"mine",
        )
        .unwrap();

        assert!(!written.conflict);
        assert_eq!(std::fs::read(dir.path().join("a.rnote")).unwrap(), b"mine");
    }

    #[test]
    fn own_notebook_changed_on_disk_goes_to_a_conflict_copy() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rnote"), b"theirs").unwrap();
        let read = Expected::Read(Some(rnote_sync::sha256_hex(b"what I opened")));

        let written = write_blocking(dir.path(), "a.rnote", read, "PC", at(), b"mine").unwrap();

        assert!(written.conflict);
        assert_eq!(
            std::fs::read(dir.path().join("a.rnote")).unwrap(),
            b"theirs"
        );
    }

    #[test]
    fn changed_on_disk_reloads_only_clean_library_notebooks() {
        assert_eq!(on_changed_on_disk(false, true), Reaction::Reload);
        assert_eq!(on_changed_on_disk(true, true), Reaction::AskAsBefore);
        assert_eq!(on_changed_on_disk(false, false), Reaction::AskAsBefore);
        assert_eq!(on_changed_on_disk(true, false), Reaction::AskAsBefore);
    }

    #[test]
    fn only_tabs_of_notebooks_the_sync_wrote_are_reloaded() {
        let root = Path::new("/mnt/Storage/HandNotes");
        let changed = vec!["Algebra/a.rnote".to_owned()];

        assert!(changed_by_sync(
            root,
            &root.join("Algebra/a.rnote"),
            &changed
        ));
        assert!(!changed_by_sync(root, &root.join("b.rnote"), &changed));
        assert!(!changed_by_sync(
            root,
            Path::new("/tmp/Algebra/a.rnote"),
            &changed
        ));
    }
}
