//! When syncs run, and where each library keeps its sync state.

use std::path::{Path, PathBuf};

/// One sync at a time: a request while one runs is kept, once, for when it ends.
#[derive(Debug, Default)]
pub(crate) struct Scheduler {
    running: bool,
    queued: bool,
}

impl Scheduler {
    /// A sync is wanted; returns whether to start it now.
    pub(crate) fn request(&mut self) -> bool {
        if self.running {
            self.queued = true;
            return false;
        }
        self.running = true;
        true
    }

    /// Whether no sync runs and none waits to.
    pub(crate) fn idle(&self) -> bool {
        !self.running && !self.queued
    }

    /// The running sync ended; returns whether to start the one asked for meanwhile.
    pub(crate) fn finished(&mut self) -> bool {
        let again = std::mem::take(&mut self.queued);
        self.running = again;
        again
    }
}

/// Where the library at `library_root` keeps its sync state, under `data_dir`: one file
/// per library, so the state of one folder is never used with another.
pub(crate) fn state_path(data_dir: &Path, library_root: &Path) -> PathBuf {
    let root = library_root
        .canonicalize()
        .unwrap_or_else(|_| library_root.to_owned());
    let hash = rnote_sync::sha256_hex(root.to_string_lossy().as_bytes());
    data_dir
        .join("rnote")
        .join("sync")
        .join(format!("{}.json", &hash[..16]))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Scheduler, state_path};

    #[test]
    fn a_request_while_running_is_chained_once() {
        let mut scheduler = Scheduler::default();

        assert!(scheduler.request());
        assert!(!scheduler.request());
        assert!(!scheduler.request());
        assert!(scheduler.finished());
        assert!(!scheduler.finished());
        assert!(scheduler.request());
    }

    #[test]
    fn idle_only_once_nothing_runs_or_waits() {
        let mut scheduler = Scheduler::default();
        assert!(scheduler.idle());

        scheduler.request();
        scheduler.request();
        assert!(!scheduler.idle());
        // The first ends, the one asked for meanwhile starts: still busy.
        scheduler.finished();
        assert!(!scheduler.idle());
        scheduler.finished();
        assert!(scheduler.idle());
    }

    #[test]
    fn state_path_differs_per_library() {
        let data = Path::new("/data");

        let a = state_path(data, Path::new("/mnt/Storage/HandNotes"));
        let b = state_path(data, Path::new("/home/me/Documents/Rnote"));

        assert_ne!(a, b);
        for path in [&a, &b] {
            assert!(path.starts_with("/data/rnote/sync"), "{path:?}");
            assert_eq!(path.extension().and_then(|e| e.to_str()), Some("json"));
            assert_eq!(path.file_stem().unwrap().len(), 16);
        }
        assert_eq!(a, state_path(data, Path::new("/mnt/Storage/HandNotes")));
    }
}
