//! Names for the copies the sync makes so that no version is ever overwritten.

use chrono::NaiveDateTime;

/// Where the other version of the notebook at `path` goes when both sides changed it:
/// `Name (conflicto, <device>, <date>).rnote` in the same folder. The date has no `:`,
/// which exFAT and Windows don't allow.
pub(crate) fn conflict_name(path: &str, device: &str, now: NaiveDateTime) -> String {
    let device: String = device
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':') || c.is_control() { '-' } else { c })
        .collect();
    let date = now.format("%Y-%m-%d %H.%M");
    with_suffix(path, &format!(" (conflicto, {device}, {date})"))
}

/// `wanted` if `taken` says it's free, or else the first free of `Name (2).ext`,
/// `Name (3).ext`…
pub(crate) fn free_path(wanted: &str, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(wanted) {
        return wanted.to_owned();
    }
    (2..)
        .map(|n| with_suffix(wanted, &format!(" ({n})")))
        .find(|path| !taken(path))
        .unwrap_or_else(|| wanted.to_owned())
}

/// `path` with `suffix` between the name and the extension.
fn with_suffix(path: &str, suffix: &str) -> String {
    let (folder, name) = match path.rsplit_once('/') {
        Some((folder, name)) => (format!("{folder}/"), name),
        None => (String::new(), path),
    };
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => {
            format!("{folder}{stem}{suffix}.{extension}")
        }
        _ => format!("{folder}{name}{suffix}"),
    }
}
