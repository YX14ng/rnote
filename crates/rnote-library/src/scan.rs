//! Finding the notebooks and folders on disk.

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::Context;

use crate::{Notebook, NotebookKind};

/// Hidden entries (a leading `.`) belong to other programs or to the library itself.
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn kind_of(name: &str) -> Option<NotebookKind> {
    let (_, extension) = name.rsplit_once('.')?;
    match extension.to_ascii_lowercase().as_str() {
        "rnote" => Some(NotebookKind::Rnote),
        "xopp" => Some(NotebookKind::Xopp),
        _ => None,
    }
}

pub(crate) fn folders(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut folders = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("Reading {}", root.display()))? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else { continue };
        if !is_hidden(&name) && entry.file_type()?.is_dir() {
            folders.push(name);
        }
    }
    Ok(folders)
}

pub(crate) fn notebooks(root: &Path) -> anyhow::Result<Vec<Notebook>> {
    let mut notebooks = in_folder(root, None)?;
    for folder in folders(root)? {
        notebooks.extend(in_folder(&root.join(&folder), Some(&folder))?);
    }
    Ok(notebooks)
}

fn in_folder(dir: &Path, folder: Option<&str>) -> anyhow::Result<Vec<Notebook>> {
    let mut notebooks = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("Reading {}", dir.display()))? {
        let entry = entry?;
        let Ok(file_name) = entry.file_name().into_string() else { continue };
        let Some(kind) = kind_of(&file_name) else { continue };
        if is_hidden(&file_name) || !entry.file_type()?.is_file() {
            continue;
        }
        let metadata = entry.metadata()?;
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
            .unwrap_or(0);
        let name = file_name.rsplit_once('.').map_or(file_name.as_str(), |(stem, _)| stem);
        notebooks.push(Notebook {
            path: folder
                .map_or_else(|| file_name.clone(), |folder| format!("{folder}/{file_name}")),
            name: name.to_owned(),
            folder: folder.map(str::to_owned),
            kind,
            modified_ms,
            size: metadata.len(),
            tags: Vec::new(),
        });
    }
    Ok(notebooks)
}
