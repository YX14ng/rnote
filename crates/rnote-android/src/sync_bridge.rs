//! The Drive sync and the saving of library notebooks, for the app: what can be tested
//! without Android lives here, and the JNI entry points only translate.

use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path};

use anyhow::{Context, bail};
use chrono::NaiveDateTime;
use rnote_sync::{Remote, SyncContext, SyncError, conflict_copy_path};
use serde_json::json;
use sha2::{Digest, Sha256};

/// A notebook the editor saved, and where it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrittenNotebook {
    /// The path written, relative to the library.
    pub path: String,
    /// The SHA-256 of what was written, in lower-case hex.
    pub sha256: String,
    /// Whether the notebook had changed since the editor read it, so the bytes went to a
    /// conflict copy beside it instead.
    pub conflict: bool,
}

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

// Only the JNI entry points, built for Android, call it outside of tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
/// Writes `bytes`, the notebook the editor has open, to `path` in the library at `root`, if
/// the file there is still the one with hash `expected_sha256` (or missing). If it changed
/// meanwhile, e.g. a sync brought another device's version, the bytes go to a conflict
/// copy beside it and the file stays as it is.
pub(crate) fn write_notebook(
    root: &Path,
    path: &str,
    expected_sha256: Option<&str>,
    device: &str,
    now: NaiveDateTime,
    bytes: &[u8],
) -> anyhow::Result<WrittenNotebook> {
    check_notebook_path(path)?;
    let current = match fs::read(root.join(path)) {
        Ok(current) => Some(current),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error).with_context(|| format!("Reading {path}")),
    };
    let changed = match (current, expected_sha256) {
        (None, _) => false,
        (Some(current), Some(expected)) => sha256_hex(&current) != expected,
        (Some(current), None) => !current.is_empty(),
    };
    let target =
        if changed { conflict_copy_path(root, path, device, now) } else { path.to_owned() };
    write_atomic(&root.join(&target), bytes)?;
    Ok(WrittenNotebook { path: target, sha256: sha256_hex(bytes), conflict: changed })
}

/// Fails unless `path` names a notebook inside the library: relative, without `..`, and
/// outside hidden folders such as `.rnote-library`.
fn check_notebook_path(path: &str) -> anyhow::Result<()> {
    let inside = Path::new(path).components().all(|component| match component {
        Component::Normal(name) => !name.to_string_lossy().starts_with('.'),
        _ => false,
    });
    if path.is_empty() || !inside || !path.ends_with(".rnote") {
        bail!("{path} isn't a notebook of the library");
    }
    Ok(())
}

/// Replaces `target` with `bytes` at once, through a hidden file in the same folder.
fn write_atomic(target: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let dir = target.parent().context("A notebook has a folder")?;
    let name = target.file_name().context("A notebook has a name")?.to_string_lossy();
    let temporary = dir.join(format!(".{name}.write-tmp"));
    fs::write(&temporary, bytes).with_context(|| format!("Writing {}", temporary.display()))?;
    fs::rename(&temporary, target).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        anyhow::Error::new(error).context(format!("Replacing {}", target.display()))
    })
}

/// The SHA-256 of `bytes`, in lower-case hex.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs::{self, File, OpenOptions};
    use std::path::{Path, PathBuf};

    use chrono::{NaiveDate, NaiveDateTime};
    use rnote_sync::fake::FakeDrive;
    use rnote_sync::{Listing, Remote, RemoteError, RemoteFile, Revision};
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::{run_sync, write_notebook};

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 11).unwrap().and_hms_opt(10, 0, 0).unwrap()
    }

    fn sha256(bytes: &[u8]) -> String {
        Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
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

    #[test]
    fn write_notebook_writes_when_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rnote"), b"old").unwrap();

        let written =
            write_notebook(dir.path(), "a.rnote", Some(&sha256(b"old")), "Tab", now(), b"new")
                .unwrap();

        assert_eq!(written.path, "a.rnote");
        assert_eq!(written.sha256, sha256(b"new"));
        assert!(!written.conflict);
        assert_eq!(fs::read(dir.path().join("a.rnote")).unwrap(), b"new");
    }

    #[test]
    fn write_notebook_keeps_a_changed_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rnote"), b"from another device").unwrap();

        let written =
            write_notebook(dir.path(), "a.rnote", Some(&sha256(b"old")), "Tab", now(), b"mine")
                .unwrap();

        assert_eq!(written.path, "a (conflicto, Tab, 2026-11-11 10.00).rnote");
        assert!(written.conflict);
        assert_eq!(fs::read(dir.path().join("a.rnote")).unwrap(), b"from another device");
        assert_eq!(fs::read(dir.path().join(&written.path)).unwrap(), b"mine");
    }

    #[test]
    fn write_notebook_creates_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Orga")).unwrap();

        let written =
            write_notebook(dir.path(), "Orga/a.rnote", Some(&sha256(b"old")), "Tab", now(), b"x")
                .unwrap();

        assert_eq!(written.path, "Orga/a.rnote");
        assert!(!written.conflict);
        assert_eq!(fs::read(dir.path().join("Orga/a.rnote")).unwrap(), b"x");
        assert_eq!(fs::read_dir(dir.path().join("Orga")).unwrap().count(), 1);
    }

    #[test]
    fn write_notebook_rejects_paths_outside() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("library");
        fs::create_dir(&root).unwrap();

        for path in ["../x.rnote", "/x.rnote", ".rnote-library/x.rnote", "Orga/../../x.rnote"] {
            assert!(write_notebook(&root, path, None, "Tab", now(), b"x").is_err(), "{path}");
        }
        assert!(!Path::new(&dir.path().join("x.rnote")).exists());
    }
}
