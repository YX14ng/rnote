//! When the two sides disagree on a path: copies so that no version is overwritten,
//! `library.json` merged instead, and names that would collide on a folder that doesn't
//! tell upper from lower case.

use std::collections::BTreeSet;

use rnote_library::{LIBRARY_DIR, META_FILE, merge_meta};

use crate::SyncError;
use crate::local::{portable_path, sha256_hex};
use crate::names::{conflict_name, free_path};
use crate::remote::RemoteFile;
use crate::run::Run;
use crate::tree::split;

/// Both sides have a version of `path`; Drive's is `file`, with content `bytes`.
pub(crate) fn both_changed(
    run: &mut Run,
    path: &str,
    file: &RemoteFile,
    bytes: &[u8],
) -> Result<(), SyncError> {
    let remote_sha256 = sha256_hex(bytes);
    let local_sha256 = run.local.get(path).cloned().unwrap_or_default();
    if remote_sha256 == local_sha256 {
        return run.record(path, file, local_sha256);
    }
    if path == meta_path() {
        return upload_meta(run, path, &file.id, Some(bytes.to_vec()));
    }
    keep_copy(run, path, bytes)?;
    upload_over(run, path, &file.id)
}

/// Uploads the local `path` over the Drive file `id`. A version another device uploaded
/// in the meantime is kept as a conflict copy (merged, for `library.json`).
pub(crate) fn upload_over(run: &mut Run, path: &str, id: &str) -> Result<(), SyncError> {
    let Some(theirs) = run.upload_update(path, id)? else {
        return Ok(());
    };
    if path == meta_path() {
        let merged = merge_meta(&run.read_local(path)?, &theirs)?;
        run.write_local(path, &merged)?;
        run.upload_update(path, id)?;
        return Ok(());
    }
    keep_copy(run, path, &theirs)
}

/// Uploads the local `library.json` over the Drive file `id`, merged first with Drive's
/// (`remote`, or downloaded), so an app that saved stale metadata can't erase what other
/// devices added.
pub(crate) fn upload_meta(
    run: &mut Run,
    path: &str,
    id: &str,
    remote: Option<Vec<u8>>,
) -> Result<(), SyncError> {
    let remote = match remote {
        Some(bytes) => bytes,
        None => run.remote.download(id)?,
    };
    let local = run.read_local(path)?;
    let merged = merge_meta(&local, &remote)?;
    if merged != local {
        run.write_local(path, &merged)?;
    }
    upload_over(run, path, id)
}

/// Keeps `bytes`, another version of `path`, as a conflict copy on both sides (unless a
/// local file already has exactly that content).
fn keep_copy(run: &mut Run, path: &str, bytes: &[u8]) -> Result<(), SyncError> {
    match local_copy_of(run, path, &sha256_hex(bytes)) {
        Some(copy) if run.state.entries.contains_key(&copy) => {}
        Some(copy) => {
            run.upload_new(&copy)?;
            run.report.conflicts.push(copy);
        }
        None => {
            let copy = free_conflict_path(run, path);
            run.write_local(&copy, bytes)?;
            run.upload_new(&copy)?;
            run.report.conflicts.push(copy);
        }
    }
    Ok(())
}

/// The other files in Drive at `path` (two devices created it without syncing), besides
/// `kept`: the same content goes to Drive's trash; a different one becomes a conflict
/// copy on both sides, or is merged in, for `library.json`.
pub(crate) fn split_duplicates(run: &mut Run, path: &str, kept: &str) -> Result<(), SyncError> {
    let kept_md5 = run.tree.file(kept).map(|(_, file)| file.md5.clone());
    let mut merged_meta = false;
    for extra in unclaimed_files_at(run, path) {
        if extra.id == kept {
            continue;
        }
        if Some(&extra.md5) == kept_md5.as_ref() {
            run.trash_remote_duplicate(&extra.id)?;
            continue;
        }
        let bytes = run.remote.download(&extra.id)?;
        if path == meta_path() {
            let merged = merge_meta(&run.read_local(path)?, &bytes)?;
            run.write_local(path, &merged)?;
            run.trash_remote_duplicate(&extra.id)?;
            merged_meta = true;
            continue;
        }
        let sha256 = sha256_hex(&bytes);
        let copy = match local_copy_of(run, path, &sha256) {
            Some(copy) if !run.state.entries.contains_key(&copy) => copy,
            _ => {
                let copy = free_conflict_path(run, path);
                run.write_local(&copy, &bytes)?;
                copy
            }
        };
        move_remote_to(run, &extra, &copy, sha256)?;
        run.report.downloaded += 1;
        run.report.conflicts.push(copy);
    }
    if merged_meta {
        upload_over(run, path, kept)?;
    }
    Ok(())
}

/// Downloads `file`, new in Drive, to `path`. If another local file has the same name but
/// for upper and lower case, or the name has characters exFAT rejects, it goes under a
/// free, portable name instead, and the Drive file is renamed to match.
pub(crate) fn download_new(run: &mut Run, path: &str, file: &RemoteFile) -> Result<(), SyncError> {
    let lower = path.to_lowercase();
    let collides = run.local.keys().any(|other| other != path && other.to_lowercase() == lower);
    let portable = portable_path(path);
    if !collides && portable == path {
        return run.download(path, file);
    }
    let target = free_path(&portable, &|candidate| taken(run, candidate));
    let bytes = run.remote.download(&file.id)?;
    let sha256 = run.write_local(&target, &bytes)?;
    move_remote_to(run, file, &target, sha256)?;
    run.report.downloaded += 1;
    Ok(())
}

/// The files in Drive at `path` that no entry of the state already accounts for, oldest
/// first.
pub(crate) fn unclaimed_files_at(run: &Run, path: &str) -> Vec<RemoteFile> {
    let claimed: BTreeSet<&str> =
        run.state.entries.values().map(|entry| entry.id.as_str()).collect();
    run.tree.files_at(path).into_iter().filter(|file| !claimed.contains(file.id.as_str())).collect()
}

/// The relative path of `library.json`.
pub(crate) fn meta_path() -> String {
    format!("{LIBRARY_DIR}/{META_FILE}")
}

/// Moves `file` in Drive to `target`, whose local copy has hash `sha256`, and records it.
fn move_remote_to(
    run: &mut Run,
    file: &RemoteFile,
    target: &str,
    sha256: String,
) -> Result<(), SyncError> {
    let (folder, name) = split(target);
    let parent = run.tree.ensure_folder(run.remote, folder)?;
    let moved = run.remote.move_file(&file.id, &file.parent, &parent, name)?;
    run.tree.put_file(target, moved.clone());
    run.record(target, &moved, sha256)
}

/// Another local file with the content of hash `sha256`, if there is one.
fn local_copy_of(run: &Run, path: &str, sha256: &str) -> Option<String> {
    run.local
        .iter()
        .find(|(other, hash)| *other != path && *hash == sha256)
        .map(|(other, _)| other.clone())
}

fn free_conflict_path(run: &Run, path: &str) -> String {
    let wanted = conflict_name(path, run.ctx.device, run.ctx.now);
    free_path(&wanted, &|candidate| taken(run, candidate))
}

/// Whether `candidate` is used here or in Drive, not telling upper from lower case.
fn taken(run: &Run, candidate: &str) -> bool {
    let lower = candidate.to_lowercase();
    run.local.keys().any(|path| path.to_lowercase() == lower)
        || run.tree.paths().iter().any(|path| path.to_lowercase() == lower)
        || run.ctx.library_root.join(candidate).exists()
}
