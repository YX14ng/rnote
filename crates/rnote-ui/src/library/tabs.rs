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

    use super::retarget;

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
