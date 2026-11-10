//! Names of notebooks and folders.

use std::path::Path;

use anyhow::bail;

/// Longest name kept, in characters.
pub const MAX_NAME_CHARS: usize = 120;

/// Checks a name typed for a notebook or folder: trims it, rejects what can't be a file
/// name in every system the library syncs to, and truncates it to [`MAX_NAME_CHARS`].
pub fn validate_name(name: &str) -> anyhow::Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("The name is empty");
    }
    if name.starts_with('.') {
        bail!("The name can't start with a dot");
    }
    if let Some(bad) = name.chars().find(|c| matches!(c, '/' | '\\' | '\0')) {
        bail!("The name can't contain {bad:?}");
    }
    Ok(name.chars().take(MAX_NAME_CHARS).collect::<String>().trim_end().to_owned())
}

/// A file name for `stem.ext` that is free in `dir`: `stem.ext`, else `stem (2).ext`, ...
///
/// Names differing only in case count as taken, since the library syncs to case-insensitive
/// file systems.
#[must_use]
pub fn unique_file_name(dir: &Path, stem: &str, ext: &str) -> String {
    let mut candidate = format!("{stem}.{ext}");
    let mut counter = 2;
    while name_taken(dir, &candidate, None) {
        candidate = format!("{stem} ({counter}).{ext}");
        counter += 1;
    }
    candidate
}

/// Whether an entry of `dir` other than `except` is named `name`, ignoring case.
pub(crate) fn name_taken(dir: &Path, name: &str, except: Option<&Path>) -> bool {
    if dir.join(name).exists() && except.is_none_or(|except| !same_file(&dir.join(name), except)) {
        return true;
    }
    let wanted = name.to_lowercase();
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    entries.flatten().any(|entry| {
        entry.file_name().to_string_lossy().to_lowercase() == wanted
            && except.is_none_or(|except| !same_file(&entry.path(), except))
    })
}

/// Whether `a` and `b` are the same file, e.g. two spellings of one name on a
/// case-insensitive file system.
pub(crate) fn same_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(a), std::fs::metadata(b)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        // Without inodes: same path but for case means the same file on the
        // case-insensitive file systems of those platforms.
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    }
}
