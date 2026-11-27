//! Showing the library again when its files change on disk: a sync, another program or
//! another window wrote there.

use std::path::PathBuf;
use std::time::Duration;

use gtk4::{gio, glib, glib::clone, prelude::*, subclass::prelude::*};
use rnote_library::{LIBRARY_DIR, META_FILE};
use tracing::debug;

use super::RnLibraryView;

/// How long to wait for more changes before showing the library again: a sync or a copy
/// changes many files in a row.
const SETTLE: Duration = Duration::from_millis(600);

/// What a watched folder is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Watched {
    /// The root, with the notebooks in no folder and the folders.
    Root,
    /// A first-level folder.
    Folder,
    /// The library's data folder.
    Data,
}

/// Whether a change to the file named `name` in a folder of kind `watched` changes what
/// the library shows. Hidden files, like the temporary ones of a write, don't.
pub(super) fn relevant(watched: Watched, name: &str) -> bool {
    match watched {
        Watched::Root => !name.starts_with('.'),
        Watched::Folder => {
            !name.starts_with('.') && (name.ends_with(".rnote") || name.ends_with(".xopp"))
        }
        Watched::Data => name == META_FILE,
    }
}

impl RnLibraryView {
    /// Watches the root, the `folders` and the library's data, instead of what was watched
    /// before.
    pub(super) fn watch(&self, folders: &[String]) {
        let root = self.root();
        let mut dirs: Vec<(PathBuf, Watched)> = vec![
            (root.clone(), Watched::Root),
            (root.join(LIBRARY_DIR), Watched::Data),
        ];
        dirs.extend(
            folders
                .iter()
                .map(|folder| (root.join(folder), Watched::Folder)),
        );
        let monitors = dirs
            .into_iter()
            .filter_map(|(dir, watched)| self.monitor(&dir, watched))
            .collect();
        for old in self.imp().monitors.replace(monitors) {
            old.cancel();
        }
    }

    fn monitor(&self, dir: &std::path::Path, watched: Watched) -> Option<gio::FileMonitor> {
        let monitor = gio::File::for_path(dir)
            .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            .map_err(|e| debug!("Can't watch {}, Err: {e:?}", dir.display()))
            .ok()?;
        monitor.connect_changed(clone!(
            #[weak(rename_to = view)]
            self,
            move |_, file, other, _| {
                let changed = [Some(file), other].into_iter().flatten().any(|file| {
                    file.basename()
                        .is_some_and(|name| relevant(watched, &name.to_string_lossy()))
                });
                if changed {
                    view.changed_on_disk();
                }
            }
        ));
        Some(monitor)
    }

    /// Shows the library again once the changes settle, if it is shown; otherwise showing
    /// it reads it again anyway.
    fn changed_on_disk(&self) {
        if self.imp().refresh_pending.replace(true) {
            return;
        }
        glib::timeout_add_local_once(
            SETTLE,
            clone!(
                #[weak(rename_to = view)]
                self,
                move || {
                    view.imp().refresh_pending.set(false);
                    if view.is_mapped() {
                        view.refresh();
                    }
                }
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Watched, relevant};

    #[test]
    fn notebooks_and_folders_at_the_root_count() {
        assert!(relevant(Watched::Root, "Álgebra"));
        assert!(relevant(Watched::Root, "suelto.rnote"));
        assert!(!relevant(Watched::Root, ".rnote-library"));
        assert!(!relevant(Watched::Root, ".suelto.rnote.tmp"));
    }

    #[test]
    fn only_notebooks_count_in_a_folder() {
        assert!(relevant(Watched::Folder, "Parcial.rnote"));
        assert!(relevant(Watched::Folder, "Orga.xopp"));
        assert!(!relevant(Watched::Folder, "apunte.pdf"));
        assert!(!relevant(Watched::Folder, ".Parcial.rnote.tmp"));
    }

    #[test]
    fn only_the_metadata_counts_in_the_data_folder() {
        assert!(relevant(Watched::Data, "library.json"));
        assert!(!relevant(Watched::Data, "library.json.tmp"));
        assert!(!relevant(Watched::Data, "trash"));
    }
}
