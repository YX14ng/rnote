//! Regressions found by the review of the sync: ways a notebook could be lost.

use std::cell::RefCell;

use super::Device;
use crate::fake::FakeDrive;
use crate::remote::{Listing, Remote, RemoteError, RemoteFile, Revision};
use crate::{SyncContext, SyncReport};

type Hook = Box<dyn FnMut(&str)>;

/// A Remote that runs `hook(op)` once, before the first call of `op_name`.
struct Hooked {
    inner: FakeDrive,
    op_name: &'static str,
    hook: RefCell<Option<Hook>>,
}

impl Hooked {
    fn fire(&self, op: &str) {
        if op == self.op_name
            && let Some(mut hook) = self.hook.borrow_mut().take()
        {
            hook(op);
        }
    }
}

impl Remote for Hooked {
    fn find_root(&mut self) -> Result<Option<String>, RemoteError> {
        self.fire("find_root");
        self.inner.find_root()
    }
    fn list(&mut self) -> Result<Listing, RemoteError> {
        self.fire("list");
        self.inner.list()
    }
    fn get(&mut self, id: &str) -> Result<Option<RemoteFile>, RemoteError> {
        self.fire("get");
        self.inner.get(id)
    }
    fn revisions(&mut self, id: &str) -> Result<Vec<Revision>, RemoteError> {
        self.fire("revisions");
        self.inner.revisions(id)
    }
    fn download_revision(&mut self, id: &str, revision: &str) -> Result<Vec<u8>, RemoteError> {
        self.fire("download_revision");
        self.inner.download_revision(id, revision)
    }
    fn download(&mut self, id: &str) -> Result<Vec<u8>, RemoteError> {
        self.fire("download");
        self.inner.download(id)
    }
    fn create_file(&mut self, p: &str, n: &str, b: &[u8]) -> Result<RemoteFile, RemoteError> {
        self.fire("create_file");
        self.inner.create_file(p, n, b)
    }
    fn update_file(&mut self, id: &str, b: &[u8]) -> Result<RemoteFile, RemoteError> {
        self.fire("update_file");
        self.inner.update_file(id, b)
    }
    fn move_file(
        &mut self,
        id: &str,
        f: &str,
        t: &str,
        n: &str,
    ) -> Result<RemoteFile, RemoteError> {
        self.fire("move_file");
        self.inner.move_file(id, f, t, n)
    }
    fn create_folder(&mut self, p: Option<&str>, n: &str) -> Result<String, RemoteError> {
        self.fire("create_folder");
        self.inner.create_folder(p, n)
    }
    fn trash(&mut self, id: &str) -> Result<(), RemoteError> {
        self.fire("trash");
        self.inner.trash(id)
    }
}

fn hooked_sync(device: &Device, drive: &FakeDrive, op: &'static str, hook: Hook) -> SyncReport {
    let ctx = SyncContext {
        library_root: device.root(),
        state_path: &device.state,
        device: device.name,
        now: super::now(),
    };
    let mut remote = Hooked { inner: drive.clone(), op_name: op, hook: RefCell::new(Some(hook)) };
    crate::sync(&ctx, &mut remote).unwrap()
}

/// Every synced local file whose content is `bytes`.
fn holders(device: &Device, bytes: &[u8]) -> Vec<String> {
    device.paths().into_iter().filter(|p| device.read(p) == bytes).collect()
}

/// Renames the Drive file at `from` (root folder) to `to`, as the Drive web UI would.
fn rename_in_drive(drive: &FakeDrive, from: &str, to: &str) {
    let mut d = drive.clone();
    let listing = d.list().unwrap();
    let file = listing.files.iter().find(|f| f.name == from).unwrap().clone();
    d.move_file(&file.id, &file.parent, &file.parent, to).unwrap();
}

#[test]
fn editor_save_during_sync_is_overwritten_by_download() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("b.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);
    a.write("b.rnote", b"phone-edit");
    a.sync(&drive);
    // B has a new notebook, so its sync uploads "a.rnote" before it reaches "b.rnote".
    b.write("a.rnote", b"new");
    let root = b.root().to_owned();
    hooked_sync(
        &b,
        &drive,
        "create_file",
        Box::new(move |_| std::fs::write(root.join("b.rnote"), b"pc-editor-save").unwrap()),
    );
    b.sync(&drive);
    a.sync(&drive);
    assert!(
        !holders(&b, b"pc-editor-save").is_empty(),
        "the save made during the sync is gone from B"
    );
}

#[test]
fn editor_save_during_sync_is_deleted() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("b.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);
    a.delete("b.rnote");
    a.sync(&drive);
    b.write("a.rnote", b"new");
    let root = b.root().to_owned();
    hooked_sync(
        &b,
        &drive,
        "create_file",
        Box::new(move |_| std::fs::write(root.join("b.rnote"), b"pc-editor-save").unwrap()),
    );
    assert!(
        !holders(&b, b"pc-editor-save").is_empty(),
        "the save made during the sync was deleted"
    );
}

#[test]
fn two_devices_updating_at_once_lose_a_version() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("n.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);
    a.write("n.rnote", b"from-A");
    b.write("n.rnote", b"from-B");
    let (a_dir, a_state) = (a.root().to_owned(), a.state.clone());
    let drive2 = drive.clone();
    // A syncs while B is between its listing and its upload.
    hooked_sync(
        &b,
        &drive,
        "update_file",
        Box::new(move |_| {
            let ctx = SyncContext {
                library_root: &a_dir,
                state_path: &a_state,
                device: "A",
                now: super::now(),
            };
            crate::sync(&ctx, &mut drive2.clone()).unwrap();
        }),
    );
    a.sync(&drive);
    b.sync(&drive);
    assert!(!holders(&a, b"from-A").is_empty(), "A's version is gone from A");
}

#[test]
fn drive_web_rename_to_non_notebook_name() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("x.rnote", b"notes");
    a.sync(&drive);
    b.sync(&drive);
    rename_in_drive(&drive, "x.rnote", "x");
    a.sync(&drive);
    a.sync(&drive);
    b.sync(&drive);
    assert!(!holders(&b, b"notes").is_empty(), "the notebook left B's library");
}

#[test]
fn duplicate_library_json_from_simultaneous_first_syncs() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    rnote_library::Library::open(a.root()).unwrap().create_tag("Parcial", 1).unwrap();
    rnote_library::Library::open(b.root()).unwrap().create_tag("Teoria", 2).unwrap();
    let (a_dir, a_state) = (a.root().to_owned(), a.state.clone());
    let drive2 = drive.clone();
    hooked_sync(
        &b,
        &drive,
        "create_file",
        Box::new(move |_| {
            let ctx = SyncContext {
                library_root: &a_dir,
                state_path: &a_state,
                device: "A",
                now: super::now(),
            };
            crate::sync(&ctx, &mut drive2.clone()).unwrap();
        }),
    );
    for _ in 0..3 {
        a.sync(&drive);
        b.sync(&drive);
    }
    let tags = |d: &Device| {
        let mut t: Vec<String> = rnote_library::Library::open(d.root())
            .unwrap()
            .tags()
            .into_iter()
            .map(|t| t.name)
            .collect();
        t.sort();
        t
    };
    let list = |d: &Device| {
        let mut v = Vec::new();
        for e in std::fs::read_dir(d.root().join(".rnote-library")).unwrap() {
            v.push(e.unwrap().file_name().to_string_lossy().into_owned());
        }
        v.sort();
        v
    };
    println!(
        "A tags {:?} files {:?}; B tags {:?} files {:?}; Drive {:?} trashed {:?}",
        tags(&a),
        list(&a),
        tags(&b),
        list(&b),
        drive.paths(),
        drive.trashed_paths()
    );
    assert_eq!(tags(&a), vec!["Parcial", "Teoria"]);
    assert_eq!(tags(&b), vec!["Parcial", "Teoria"]);
}

#[test]
fn simultaneous_first_syncs_make_two_roots() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("from-a.rnote", b"a");
    b.write("from-b.rnote", b"b");
    let (a_dir, a_state) = (a.root().to_owned(), a.state.clone());
    let drive2 = drive.clone();
    // A's first sync runs while B is between finding no `Rnote` folder and creating one.
    hooked_sync(
        &b,
        &drive,
        "create_folder",
        Box::new(move |_| {
            let ctx = SyncContext {
                library_root: &a_dir,
                state_path: &a_state,
                device: "A",
                now: super::now(),
            };
            crate::sync(&ctx, &mut drive2.clone()).unwrap();
        }),
    );
    for _ in 0..3 {
        a.sync(&drive);
        b.sync(&drive);
    }
    assert_eq!(a.paths(), b.paths(), "the two devices never meet");
}

#[test]
fn crash_after_local_move_before_state_save() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("x.rnote", b"notes");
    a.sync(&drive);
    b.sync(&drive);
    rename_in_drive(&drive, "x.rnote", "y.rnote");
    // B applied the move on disk and was killed before saving the state.
    std::fs::rename(b.root().join("x.rnote"), b.root().join("y.rnote")).unwrap();
    let report = b.sync(&drive);
    a.sync(&drive);
    assert_eq!(report.deleted, 0);
    assert_eq!(a.paths(), vec!["y.rnote"]);
    assert_eq!(b.paths(), vec!["y.rnote"]);
}

#[test]
fn library_folder_changed_or_unmounted_wipes_other_devices() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("Algebra/Parcial 1.rnote", b"p1");
    a.write("Orga/x.rnote", b"x");
    a.sync(&drive);
    b.sync(&drive);
    // A's library setting now points at another (empty) folder, or the disk isn't mounted
    // and the app re-created the folder: same state file, empty library.
    let other = tempfile::tempdir().unwrap();
    rnote_library::Library::open(other.path()).unwrap();
    let ctx = SyncContext {
        library_root: other.path(),
        state_path: &a.state,
        device: "A",
        now: super::now(),
    };
    let report = crate::sync(&ctx, &mut drive.clone()).unwrap();
    b.sync(&drive);
    println!(
        "A report {report:?}; B {:?}; Drive {:?}; trashed {:?}",
        b.paths(),
        drive.paths(),
        drive.trashed_paths()
    );
    assert!(!b.paths().is_empty(), "B's notebooks were deleted");
}

#[test]
fn remote_names_cannot_reach_outside_the_library() {
    let drive = FakeDrive::new();
    let name = format!("evil-{}.rnote", std::process::id());
    drive.add_file(&format!("../{name}"), b"x");
    drive.add_file("a\\b.rnote", b"x");
    let a = Device::new("A");

    a.sync(&drive);

    let outside = a.root().parent().unwrap().join(&name);
    let escaped = outside.exists();
    let _ = std::fs::remove_file(&outside);
    assert!(!escaped, "a remote name wrote outside the library");
    assert!(a.paths().is_empty());
}

#[test]
fn a_notebook_moved_out_of_rnote_in_drive_stays_in_the_library() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    a.write("x.rnote", b"notes");
    a.sync(&drive);
    let mut d = drive.clone();
    let other = d.create_folder(None, "Other").unwrap();
    let file = d.list().unwrap().files.into_iter().find(|f| f.name == "x.rnote").unwrap();
    d.move_file(&file.id, &file.parent, &other, "x.rnote").unwrap();

    a.sync(&drive);

    assert_eq!(a.read("x.rnote"), b"notes");
    assert_eq!(drive.paths(), vec!["x.rnote"]);
}

#[test]
fn names_exfat_rejects_are_renamed_on_both_sides() {
    let drive = FakeDrive::new();
    drive.add_file("Clase 1: repaso?.rnote", b"c");
    let a = Device::new("A");

    a.sync(&drive);

    assert_eq!(a.read("Clase 1- repaso-.rnote"), b"c");
    assert_eq!(drive.paths(), vec!["Clase 1- repaso-.rnote"]);
}

#[test]
fn a_file_that_cant_be_written_is_skipped_and_the_rest_syncs() {
    let drive = FakeDrive::new();
    drive.add_file("Orga/a.rnote", b"a");
    drive.add_file("z.rnote", b"z");
    let a = Device::new("A");
    a.write("b.rnote", b"b");
    std::fs::write(a.root().join("Orga"), b"a file where a folder should be").unwrap();

    let report = a.sync(&drive);

    assert_eq!(report.skipped.len(), 1, "{report:?}");
    assert!(report.skipped[0].starts_with("Orga/a.rnote"), "{report:?}");
    assert_eq!(a.read("z.rnote"), b"z");
    assert!(drive.paths().contains(&"b.rnote".to_owned()));
}

#[test]
fn a_stale_library_json_written_by_the_app_doesnt_erase_other_devices_tags() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    rnote_library::Library::open(a.root()).unwrap().create_tag("Parcial", 1).unwrap();
    a.sync(&drive);
    // B's app has its library open from before the sync and saves it afterwards.
    let mut stale = rnote_library::Library::open(b.root()).unwrap();
    b.sync(&drive);
    stale.create_tag("Teoria", 2).unwrap();
    b.sync(&drive);
    a.sync(&drive);

    for device in [&a, &b] {
        let mut names: Vec<String> = rnote_library::Library::open(device.root())
            .unwrap()
            .tags()
            .into_iter()
            .map(|t| t.name)
            .collect();
        names.sort();
        assert_eq!(names, vec!["Parcial", "Teoria"], "device {}", device.name);
    }
}

#[test]
fn the_report_lists_what_changed_here() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("x.rnote", b"1");
    a.write("y.rnote", b"1");
    a.sync(&drive);
    b.sync(&drive);
    a.write("x.rnote", b"2");
    a.delete("y.rnote");
    a.sync(&drive);

    let report = b.sync(&drive);

    assert_eq!(report.changed_here, vec!["x.rnote", "y.rnote"]);
}

#[test]
fn local_deletions_keep_a_copy_outside_the_library() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("x.rnote", b"keep me");
    a.sync(&drive);
    b.sync(&drive);
    a.delete("x.rnote");
    a.sync(&drive);

    b.sync(&drive);

    assert!(b.paths().is_empty());
    let removed = b.root().join(".rnote-library/.sync-removed");
    let kept: Vec<_> = std::fs::read_dir(removed)
        .unwrap()
        .map(|e| std::fs::read(e.unwrap().path()).unwrap())
        .collect();
    assert_eq!(kept, vec![b"keep me".to_vec()]);
}

#[test]
fn a_device_without_its_library_marker_never_deletes() {
    let drive = FakeDrive::new();
    let a = Device::new("A");
    let b = Device::new("B");
    a.write("x.rnote", b"x");
    a.sync(&drive);
    b.sync(&drive);
    // A's library folder was replaced by an empty one at the same place.
    std::fs::remove_dir_all(a.root()).unwrap();
    std::fs::create_dir(a.root()).unwrap();

    let report = a.sync(&drive);
    b.sync(&drive);

    assert_eq!(report.deleted, 0);
    assert_eq!(a.read("x.rnote"), b"x");
    assert_eq!(b.read("x.rnote"), b"x");
}

/// The editor's save and the sync's write of the same file never interleave their check
/// and their write: the sync waits for the library's write guard.
#[test]
fn sync_writes_wait_for_the_write_guard() {
    let device = Device::new("Tab");
    let root = device.root().to_owned();
    let held = rnote_library::write_guard();

    let syncer = std::thread::spawn(move || {
        let drive = FakeDrive::new();
        drive.add_file("a.rnote", b"remote");
        device.sync(&drive);
        device
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    let written_while_held = root.join("a.rnote").exists();
    drop(held);
    let device = syncer.join().unwrap();

    assert!(!written_while_held);
    assert_eq!(device.read("a.rnote"), b"remote");
}
