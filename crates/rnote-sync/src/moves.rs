//! Moves and renames, found before the content is compared so that a moved notebook is
//! moved on the other side instead of being deleted and sent again.
//!
//! In Drive a file keeps its id when it moves, so a move there shows as a known id at
//! another path. Here a file has no id, so a move shows as a known file gone from its path
//! and a new file with the same content somewhere else.

use std::collections::BTreeSet;
use std::fs;

use anyhow::Context;

use crate::SyncError;
use crate::conflicts::meta_path;
use crate::content::skip_on_failure;
use crate::local::portable_path;
use crate::names::free_path;
use crate::run::Run;
use crate::tree::split;

/// Applies the moves made in Drive here, and the moves made here in Drive.
pub(crate) fn apply_moves(run: &mut Run) -> Result<(), SyncError> {
    remote_moves(run)?;
    local_moves(run)
}

/// A file of the state that Drive now has at another path is moved here too. If that path
/// is taken here, or not portable, it lands under a free name, and Drive's file is renamed
/// to match.
fn remote_moves(run: &mut Run) -> Result<(), SyncError> {
    let entries: Vec<(String, String)> =
        run.state.entries.iter().map(|(path, entry)| (path.clone(), entry.id.clone())).collect();
    for (old, id) in entries {
        let result = remote_move(run, &old, &id);
        skip_on_failure(run, &old, result)?;
    }
    Ok(())
}

fn remote_move(run: &mut Run, old: &str, id: &str) -> Result<(), SyncError> {
    let Some((new, file)) = run.tree.file(id).map(|(path, file)| (path.to_owned(), file.clone()))
    else {
        return Ok(());
    };
    // `library.json` is merged, never moved.
    if new == old || old == meta_path() || new == meta_path() {
        return Ok(());
    }
    let wanted = portable_path(&new);
    let target = if wanted != new || taken_here(run, &new, old) {
        free_path(&wanted, &|candidate| taken_here(run, candidate, old))
    } else {
        new.clone()
    };
    if run.local.contains_key(old) {
        let _guard = rnote_library::write_guard();
        run.expect_local(old)?;
        move_local(run, old, &target)?;
    }
    if target != new {
        let (folder, name) = split(&target);
        let parent = run.tree.ensure_folder(run.remote, folder)?;
        let moved = run.remote.move_file(id, &file.parent, &parent, name)?;
        run.tree.put_file(&target, moved);
    }
    if let Some(entry) = run.state.entries.remove(old) {
        run.state.entries.insert(target, entry);
    }
    run.report.moved += 1;
    run.save()
}

/// A file of the state gone from here, unchanged in Drive, whose content turned up at a
/// path neither side knows: moved in Drive too, without sending it again.
fn local_moves(run: &mut Run) -> Result<(), SyncError> {
    let mut unknown: Vec<(String, String)> = run
        .local
        .iter()
        .filter(|(path, _)| {
            !run.state.entries.contains_key(*path) && run.tree.files_at(path).is_empty()
        })
        .map(|(path, sha256)| (path.clone(), sha256.clone()))
        .collect();
    let gone: Vec<(String, crate::state::Entry)> = run
        .state
        .entries
        .iter()
        .filter(|(path, _)| !run.local.contains_key(*path))
        .map(|(path, entry)| (path.clone(), entry.clone()))
        .collect();
    for (old, entry) in gone {
        if old == meta_path() {
            continue;
        }
        let Some(file) = run
            .tree
            .file(&entry.id)
            .filter(|(path, file)| *path == old && file.md5 == entry.md5)
            .map(|(_, file)| file.clone())
        else {
            continue;
        };
        let Some(index) = unknown.iter().position(|(_, sha256)| *sha256 == entry.sha256) else {
            continue;
        };
        let (new, sha256) = unknown.remove(index);
        let result = local_move(run, &old, &new, &file, sha256);
        skip_on_failure(run, &old, result)?;
    }
    Ok(())
}

fn local_move(
    run: &mut Run,
    old: &str,
    new: &str,
    file: &crate::RemoteFile,
    sha256: String,
) -> Result<(), SyncError> {
    let (folder, name) = split(new);
    let parent = run.tree.ensure_folder(run.remote, folder)?;
    let moved = run.remote.move_file(&file.id, &file.parent, &parent, name)?;
    run.tree.put_file(new, moved.clone());
    run.state.entries.remove(old);
    run.report.moved += 1;
    run.record(new, &moved, sha256)
}

/// Moves the local file `from` to `to`, creating its folder.
fn move_local(run: &mut Run, from: &str, to: &str) -> Result<(), SyncError> {
    let target = run.ctx.library_root.join(to);
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).with_context(|| format!("Creating {}", dir.display()))?;
    }
    fs::rename(run.ctx.library_root.join(from), &target)
        .with_context(|| format!("Moving {from} to {to}"))?;
    if let Some(sha256) = run.local.remove(from) {
        run.local.insert(to.to_owned(), sha256);
    }
    run.emptied.insert(split(from).0.to_owned());
    run.report.changed_here.extend([from.to_owned(), to.to_owned()]);
    Ok(())
}

/// Whether `candidate` is taken here by something other than `moving`, not telling upper
/// from lower case.
fn taken_here(run: &Run, candidate: &str, moving: &str) -> bool {
    let lower = candidate.to_lowercase();
    let known: BTreeSet<&String> = run.local.keys().chain(run.state.entries.keys()).collect();
    known.into_iter().any(|path| path != moving && path.to_lowercase() == lower)
        || (candidate != moving && run.ctx.library_root.join(candidate).exists())
}
