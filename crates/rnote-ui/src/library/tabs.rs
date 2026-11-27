//! Keeping the tabs right when the library moves their files.

use std::path::{Path, PathBuf};

use gtk4::{gio, prelude::*};

use crate::{RnAppWindow, RnCanvasWrapper};

/// Where `path` is after `from` moved to `to`: `to` for `from` itself, the same place under
/// `to` for a path inside the folder `from`, else `None`. Compares whole components, so
/// `/a/bc` is not inside `/a/b`.
pub(crate) fn retarget(path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(from).ok()?;
    Some(if rest.as_os_str().is_empty() {
        to.to_owned()
    } else {
        to.join(rest)
    })
}

/// The tabs of every window of the app: a tab dragged out to another window still has its
/// file in the library.
fn all_tabs(appwindow: &RnAppWindow) -> Vec<RnCanvasWrapper> {
    appwindow
        .app()
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<RnAppWindow>().ok())
        .flat_map(|window| window.get_all_tabs())
        .collect()
}

/// Points the tabs with `from`, or a file inside the folder `from`, at the same file
/// under `to`. Called right after the move, before the main loop runs again, so the tabs'
/// file watchers are replaced before they see the move.
pub(crate) fn retarget_open_tabs(appwindow: &RnAppWindow, from: &Path, to: &Path) {
    for wrapper in all_tabs(appwindow) {
        let canvas = wrapper.canvas();
        let Some(path) = canvas.output_file().and_then(|file| file.path()) else {
            continue;
        };
        if let Some(new_path) = retarget(&path, from, to) {
            canvas.set_output_file(Some(gio::File::for_path(new_path)));
        }
    }
}

/// Where the file at `path` is after the moves a sync made in the library at `root`, given
/// as (from, to) relative paths in their order; `None` if it didn't move.
pub(crate) fn followed(path: &Path, root: &Path, moves: &[(String, String)]) -> Option<PathBuf> {
    let mut current = path.to_owned();
    for (from, to) in moves {
        if current == root.join(from) {
            current = root.join(to);
        }
    }
    (current != path).then_some(current)
}

/// Points the tabs whose files a sync moved in the library at `root` at their new places.
/// A move to another folder looks to a tab like its file was removed, so it lost the file
/// meanwhile: it gets it back, with the unsaved changes it had then.
pub(crate) fn follow_sync_moves(appwindow: &RnAppWindow, root: &Path, moves: &[(String, String)]) {
    if moves.is_empty() {
        return;
    }
    for wrapper in all_tabs(appwindow) {
        let canvas = wrapper.canvas();
        if let Some(path) = canvas.output_file().and_then(|file| file.path()) {
            if let Some(new_path) = followed(&path, root, moves) {
                canvas.set_output_file(Some(gio::File::for_path(new_path)));
            }
        } else if let Some((path, unsaved)) = canvas.moved_away()
            && let Some(new_path) = followed(&path, root, moves)
        {
            canvas.set_moved_away(None);
            canvas.set_output_file(Some(gio::File::for_path(new_path)));
            canvas.set_unsaved_changes(unsaved);
        }
    }
}

/// A sync wrote the library notebooks at `changed`: the tabs showing them load the new
/// version if they have nothing unsaved; with unsaved changes, saving keeps both versions.
pub(crate) fn reload_synced(appwindow: &RnAppWindow, root: &Path, changed: &[String]) {
    if changed.is_empty() {
        return;
    }
    for wrapper in all_tabs(appwindow) {
        let canvas = wrapper.canvas();
        let synced = canvas
            .output_file()
            .and_then(|file| file.path())
            .is_some_and(|path| crate::sync::opened::changed_by_sync(root, &path, changed));
        if synced {
            crate::sync::opened::reload_if_clean(appwindow, &canvas);
        }
    }
}

/// The name of a tab with `path`, or a file inside the folder `path`, if one is open.
pub(crate) fn open_tab_inside(appwindow: &RnAppWindow, path: &Path) -> Option<String> {
    all_tabs(appwindow).into_iter().find_map(|wrapper| {
        let file_path = wrapper.canvas().output_file()?.path()?;
        retarget(&file_path, path, path)?;
        Some(
            file_path
                .file_stem()
                .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned()),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{followed, retarget};

    #[test]
    fn a_file_a_sync_moved_is_followed_to_its_new_place() {
        let moves = [
            ("Orga/a.rnote".to_owned(), "b.rnote".to_owned()),
            ("b.rnote".to_owned(), "F/c.rnote".to_owned()),
        ];

        assert_eq!(
            followed(Path::new("/l/Orga/a.rnote"), Path::new("/l"), &moves),
            Some(PathBuf::from("/l/F/c.rnote"))
        );
        assert_eq!(
            followed(Path::new("/l/x.rnote"), Path::new("/l"), &moves),
            None
        );
        assert_eq!(
            followed(Path::new("/other/Orga/a.rnote"), Path::new("/l"), &moves),
            None
        );
    }

    #[test]
    fn retarget_maps_the_file_itself() {
        let to = retarget(
            Path::new("/l/a.rnote"),
            Path::new("/l/a.rnote"),
            Path::new("/l/F/b.rnote"),
        );

        assert_eq!(to, Some(PathBuf::from("/l/F/b.rnote")));
    }

    #[test]
    fn retarget_maps_files_inside_a_renamed_folder() {
        let to = retarget(
            Path::new("/l/Algebra/p1.rnote"),
            Path::new("/l/Algebra"),
            Path::new("/l/Álgebra"),
        );

        assert_eq!(to, Some(PathBuf::from("/l/Álgebra/p1.rnote")));
    }

    #[test]
    fn retarget_ignores_a_sibling_with_a_common_prefix() {
        assert_eq!(
            retarget(
                Path::new("/l/abc/x.rnote"),
                Path::new("/l/ab"),
                Path::new("/l/z")
            ),
            None
        );
    }

    #[test]
    fn retarget_ignores_unrelated_paths() {
        assert_eq!(
            retarget(
                Path::new("/other/x.rnote"),
                Path::new("/l/x.rnote"),
                Path::new("/l/y.rnote")
            ),
            None
        );
    }
}
