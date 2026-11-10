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
#[must_use]
pub fn unique_file_name(dir: &Path, stem: &str, ext: &str) -> String {
    let mut candidate = format!("{stem}.{ext}");
    let mut counter = 2;
    while dir.join(&candidate).exists() {
        candidate = format!("{stem} ({counter}).{ext}");
        counter += 1;
    }
    candidate
}
