//! Comparing each path on three sides (this device, Drive and the last sync) and doing
//! what makes both sides equal without losing a version.

use std::collections::BTreeSet;
use std::fs;

use crate::SyncError;
use crate::conflicts::{both_changed, download_new, split_duplicates, unclaimed_files_at};
use crate::local::sha256_hex;
use crate::run::Run;
use crate::tree::split;

/// Makes the content of every path equal on both sides.
pub(crate) fn reconcile(run: &mut Run) -> Result<(), SyncError> {
    let mut paths: BTreeSet<String> = run.local.keys().cloned().collect();
    paths.extend(run.state.entries.keys().cloned());
    paths.extend(run.tree.paths());
    for path in paths {
        reconcile_path(run, &path)?;
    }
    Ok(())
}

fn reconcile_path(run: &mut Run, path: &str) -> Result<(), SyncError> {
    let local = run.local.get(path).cloned();
    let Some(entry) = run.state.entries.get(path).cloned() else {
        return reconcile_new(run, path, local);
    };
    let remote = run
        .tree
        .file(&entry.id)
        .filter(|(remote_path, _)| *remote_path == path)
        .map(|(_, file)| file.clone());
    match (local, remote) {
        (Some(sha256), Some(file)) => {
            let local_changed = sha256 != entry.sha256;
            let remote_changed = file.md5 != entry.md5;
            match (local_changed, remote_changed) {
                (false, false) => {}
                (true, false) => run.upload_update(path, &file.id)?,
                (false, true) => run.download(path, &file)?,
                (true, true) => {
                    let bytes = run.remote.download(&file.id)?;
                    both_changed(run, path, &file, &bytes)?;
                }
            }
        }
        // Gone from Drive: a local edit wins and comes back; otherwise it goes here too.
        (Some(sha256), None) => {
            if sha256 == entry.sha256 {
                run.delete_local(path)?;
            } else {
                run.forget(path)?;
                run.upload_new(path)?;
            }
        }
        // Gone from here: an edit in Drive wins and comes back; otherwise it goes there too.
        (None, Some(file)) => {
            if file.md5 == entry.md5 {
                run.trash_remote(path, &file.id)?;
            } else {
                run.download(path, &file)?;
            }
        }
        (None, None) => run.forget(path)?,
    }
    // Files another device put at the same path meanwhile.
    match run.state.entries.get(path).map(|entry| entry.id.clone()) {
        Some(kept) => split_duplicates(run, path, &kept),
        None if !unclaimed_files_at(run, path).is_empty() => {
            let local = run.local.get(path).cloned();
            reconcile_new(run, path, local)
        }
        None => Ok(()),
    }
}

/// A path the last sync didn't see.
fn reconcile_new(run: &mut Run, path: &str, local: Option<String>) -> Result<(), SyncError> {
    let Some(file) = unclaimed_files_at(run, path).into_iter().next() else {
        return match local {
            Some(_) => run.upload_new(path),
            None => Ok(()),
        };
    };
    match local {
        None => download_new(run, path, &file)?,
        Some(sha256) => {
            let bytes = run.remote.download(&file.id)?;
            if sha256_hex(&bytes) == sha256 {
                run.record(path, &file, sha256)?;
            } else {
                both_changed(run, path, &file, &bytes)?;
            }
        }
    }
    match run.state.entries.get(path).map(|entry| entry.id.clone()) {
        Some(kept) => split_duplicates(run, path, &kept),
        None => Ok(()),
    }
}

/// Trashes the folders in Drive that were left without files and aren't here, and
/// deletes the local folders this sync emptied that aren't in Drive.
pub(crate) fn clean_folders(run: &mut Run) -> Result<(), SyncError> {
    let files = run.tree.paths();
    let has_files = |folder: &str| {
        let prefix = format!("{folder}/");
        files.iter().any(|file| file.starts_with(&prefix))
    };
    let mut trashed: Vec<String> = Vec::new();
    for folder in run.tree.folder_paths() {
        let under_trashed = trashed.iter().any(|done| folder.starts_with(&format!("{done}/")));
        if under_trashed
            || hidden(&folder)
            || has_files(&folder)
            || run.ctx.library_root.join(&folder).is_dir()
        {
            continue;
        }
        if let Some(id) = run.tree.folder_id(&folder).map(str::to_owned) {
            run.remote.trash(&id)?;
        }
        run.tree.remove_folder(&folder);
        trashed.push(folder);
    }
    let emptied: Vec<String> = run.emptied.iter().cloned().collect();
    for mut folder in emptied {
        while !folder.is_empty() && !hidden(&folder) && run.tree.folder_id(&folder).is_none() {
            let dir = run.ctx.library_root.join(&folder);
            let empty = fs::read_dir(&dir).is_ok_and(|mut entries| entries.next().is_none());
            if !empty || fs::remove_dir(&dir).is_err() {
                break;
            }
            folder = split(&folder).0.to_owned();
        }
    }
    Ok(())
}

/// Whether a part of `path` is hidden (the library's own folder, for one).
fn hidden(path: &str) -> bool {
    path.split('/').any(|part| part.starts_with('.'))
}
