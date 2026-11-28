//! Where notebooks and folders of the library were moved lately in this process (renamed,
//! moved to a folder, or moved by a sync), so a save that still has the old path, made
//! before the editor heard of the move, goes to the new place instead of bringing a copy
//! back at the old one.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// How many moves are remembered: only the last few matter, for saves already under way.
const REMEMBERED: usize = 64;

static MOVES: Mutex<Vec<(PathBuf, PathBuf)>> = Mutex::new(Vec::new());

/// `from` (a notebook or a folder) is now `to`.
pub fn record_move(from: &Path, to: &Path) {
    let mut moves = MOVES.lock().unwrap_or_else(PoisonError::into_inner);
    if moves.len() == REMEMBERED {
        moves.remove(0);
    }
    moves.push((from.to_owned(), to.to_owned()));
}

/// Where `path` is now, following the moves of it or of a folder holding it, in order;
/// `None` if none of them moved it.
pub fn moved_to(path: &Path) -> Option<PathBuf> {
    let moves = MOVES.lock().unwrap_or_else(PoisonError::into_inner);
    let mut current = path.to_owned();
    let mut moved = false;
    for (from, to) in moves.iter() {
        if let Ok(rest) = current.strip_prefix(from) {
            current = if rest.as_os_str().is_empty() { to.clone() } else { to.join(rest) };
            moved = true;
        }
    }
    moved.then_some(current)
}
