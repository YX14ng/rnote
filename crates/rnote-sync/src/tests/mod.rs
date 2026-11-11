//! Tests of the sync, against the in-memory Drive and real folders in a temporary folder.

#![allow(clippy::unwrap_used)]

mod basic;
mod conflicts;
mod devices;
mod drive;
mod fake;
mod local;
mod moves;
mod regressions;
mod stub_http;

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveDateTime};
use tempfile::TempDir;

use crate::fake::FakeDrive;
use crate::{SyncContext, SyncError, SyncReport};

/// The time every simulated device syncs at.
pub(super) fn now() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 10).unwrap().and_hms_opt(21, 50, 0).unwrap()
}

/// A simulated device: a library folder and a sync state outside it.
pub(super) struct Device {
    pub dir: TempDir,
    pub state: PathBuf,
    _state_dir: TempDir,
    pub name: &'static str,
}

impl Device {
    pub fn new(name: &'static str) -> Self {
        let state_dir = tempfile::tempdir().unwrap();
        let state = state_dir.path().join("sync-state.json");
        Self { dir: tempfile::tempdir().unwrap(), state, _state_dir: state_dir, name }
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn try_sync(&self, drive: &FakeDrive) -> Result<SyncReport, SyncError> {
        let ctx = SyncContext {
            library_root: self.root(),
            state_path: &self.state,
            device: self.name,
            now: now(),
        };
        crate::sync(&ctx, &mut drive.clone())
    }

    pub fn sync(&self, drive: &FakeDrive) -> SyncReport {
        self.try_sync(drive).unwrap()
    }

    pub fn write(&self, path: &str, bytes: &[u8]) {
        write(self.root(), path, bytes);
    }

    pub fn read(&self, path: &str) -> Vec<u8> {
        fs::read(self.root().join(path)).unwrap()
    }

    pub fn delete(&self, path: &str) {
        fs::remove_file(self.root().join(path)).unwrap();
    }

    /// The synced files of the device, sorted.
    pub fn paths(&self) -> Vec<String> {
        crate::local::scan(self.root()).unwrap().into_keys().collect()
    }
}

/// Writes `contents` to `root/relative`, creating the folders on the way.
pub(super) fn write(root: &Path, relative: &str, contents: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}
