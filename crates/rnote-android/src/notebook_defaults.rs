//! The last setup used in each folder, so the next notebook there starts the same. Kept
//! on the device, outside the library: it is never synced.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Context;

use crate::notebook_setup::NotebookSetup;

/// The key of the notebooks outside any folder.
const ROOT_KEY: &str = "";

/// The setup last remembered for `folder` (`None` = outside any folder) in `file`; the
/// default one if there is none or the file can't be read.
pub(crate) fn defaults_for(file: &Path, folder: Option<&str>) -> NotebookSetup {
    read(file).remove(folder.unwrap_or(ROOT_KEY)).unwrap_or_default()
}

/// Remembers `setup` for `folder` in `file`, replacing the file at once.
pub(crate) fn remember(
    file: &Path,
    folder: Option<&str>,
    setup: &NotebookSetup,
) -> anyhow::Result<()> {
    let mut all = read(file);
    all.insert(folder.unwrap_or(ROOT_KEY).to_owned(), *setup);
    let temporary = file.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&all)?)
        .with_context(|| format!("Writing {}", temporary.display()))?;
    fs::rename(&temporary, file).with_context(|| format!("Replacing {}", file.display()))
}

fn read(file: &Path) -> BTreeMap<String, NotebookSetup> {
    fs::read(file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

#[cfg(test)]
mod tests;
