//! The Drive sync, for the app: what can be tested without Android lives here, and the JNI
//! entry points only translate. Saving library notebooks lives in `rnote_sync`, which the
//! desktop shares.

use std::path::Path;

use chrono::NaiveDateTime;
use rnote_sync::{Remote, SyncContext, SyncError};
use serde_json::json;

// Only the JNI entry points, built for Android, call it outside of tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
/// Syncs the library at `root` with `remote` and answers in JSON: `{"status": "ok", ...}`
/// with what the sync did, or `{"status": "busy" | "unauthorized" | "offline" | "failed"}`
/// with a `message` for the last two. Never fails: the outcome travels in the JSON.
pub(crate) fn run_sync(
    root: &Path,
    state_path: &Path,
    device: &str,
    remote: &mut dyn Remote,
    now: NaiveDateTime,
) -> String {
    let ctx = SyncContext { library_root: root, state_path, device, now };
    let answer = match rnote_sync::sync(&ctx, remote) {
        Ok(report) => json!({
            "status": "ok",
            "uploaded": report.uploaded,
            "downloaded": report.downloaded,
            "moved": report.moved,
            "deleted": report.deleted,
            "conflicts": report.conflicts,
            "changed_here": report.changed_here,
            "skipped": report.skipped,
            "moved_here": report
                .moved_here
                .iter()
                .map(|(from, to)| json!({ "from": from, "to": to }))
                .collect::<Vec<_>>(),
        }),
        Err(SyncError::Busy) => json!({ "status": "busy" }),
        Err(SyncError::Unauthorized) => json!({ "status": "unauthorized" }),
        Err(SyncError::Offline(message)) => json!({ "status": "offline", "message": message }),
        // `{:#}` keeps the whole context chain in a single line.
        Err(SyncError::Failed(error)) => {
            json!({ "status": "failed", "message": format!("{error:#}") })
        }
    };
    answer.to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs::{self, File, OpenOptions};
    use std::path::PathBuf;

    use chrono::{NaiveDate, NaiveDateTime};
    use rnote_sync::fake::FakeDrive;
    use rnote_sync::{Listing, Remote, RemoteError, RemoteFile, Revision};
    use serde_json::Value;
    use tempfile::TempDir;

    use super::run_sync;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 11).unwrap().and_hms_opt(10, 0, 0).unwrap()
    }

    /// A library folder and a state path outside it.
    struct Device {
        dir: TempDir,
    }

    impl Device {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            fs::create_dir(dir.path().join("library")).unwrap();
            Self { dir }
        }

        fn root(&self) -> PathBuf {
            self.dir.path().join("library")
        }

        fn state(&self) -> PathBuf {
            self.dir.path().join("sync").join("state.json")
        }

        fn sync(&self, remote: &mut dyn Remote) -> Value {
            let answer = run_sync(&self.root(), &self.state(), "Tab", remote, now());
            serde_json::from_str(&answer).unwrap()
        }
    }

    /// A Drive that refuses the token for everything.
    struct Refusing;

    impl Remote for Refusing {
        fn find_root(&mut self) -> Result<Option<String>, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn list(&mut self) -> Result<Listing, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn get(&mut self, _: &str) -> Result<Option<RemoteFile>, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn revisions(&mut self, _: &str) -> Result<Vec<Revision>, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn download_revision(&mut self, _: &str, _: &str) -> Result<Vec<u8>, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn download(&mut self, _: &str) -> Result<Vec<u8>, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn create_file(&mut self, _: &str, _: &str, _: &[u8]) -> Result<RemoteFile, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn update_file(&mut self, _: &str, _: &[u8]) -> Result<RemoteFile, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn move_file(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<RemoteFile, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn create_folder(&mut self, _: Option<&str>, _: &str) -> Result<String, RemoteError> {
            Err(RemoteError::Unauthorized)
        }
        fn trash(&mut self, _: &str) -> Result<(), RemoteError> {
            Err(RemoteError::Unauthorized)
        }
    }

    #[test]
    fn ok_reports_counts_and_changed_paths() {
        let device = Device::new();
        let mut drive = FakeDrive::new();
        drive.add_file("a.rnote", b"a");

        let answer = device.sync(&mut drive);

        assert_eq!(answer["status"], "ok");
        assert_eq!(answer["downloaded"], 1);
        assert_eq!(answer["uploaded"], 0);
        assert_eq!(answer["changed_here"], serde_json::json!(["a.rnote"]));
        assert_eq!(answer["conflicts"], serde_json::json!([]));
        assert_eq!(answer["skipped"], serde_json::json!([]));
        assert_eq!(answer["moved_here"], serde_json::json!([]));
        assert_eq!(fs::read(device.root().join("a.rnote")).unwrap(), b"a");
    }

    #[test]
    fn busy_is_reported() {
        let device = Device::new();
        fs::create_dir_all(device.state().parent().unwrap()).unwrap();
        let mut lock_path = device.state().into_os_string();
        lock_path.push(".lock");
        let held: File =
            OpenOptions::new().create(true).truncate(false).write(true).open(&lock_path).unwrap();
        held.try_lock().unwrap();

        let answer = device.sync(&mut FakeDrive::new());

        assert_eq!(answer, serde_json::json!({"status": "busy"}));
    }

    #[test]
    fn offline_is_reported() {
        let device = Device::new();
        let mut drive = FakeDrive::new();
        drive.fail_after(0);

        let answer = device.sync(&mut drive);

        assert_eq!(answer["status"], "offline");
        assert!(!answer["message"].as_str().unwrap().is_empty());
    }

    #[test]
    fn unauthorized_is_reported() {
        let device = Device::new();

        let answer = device.sync(&mut Refusing);

        assert_eq!(answer, serde_json::json!({"status": "unauthorized"}));
    }
}
