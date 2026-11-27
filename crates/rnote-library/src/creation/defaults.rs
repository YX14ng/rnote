//! The last setup used in each folder, so the next notebook there starts the same. Kept
//! on the device, outside the library: it is never synced.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Context;

use super::setup::{DARK_LINES, DARK_PAPER, NotebookSetup};

/// The key of the notebooks outside any folder.
const ROOT_KEY: &str = "";

/// The setup last remembered for `folder` (`None` = outside any folder) in `file`; the
/// default one if there is none or the file can't be read.
pub fn defaults_for(file: &Path, folder: Option<&str>) -> NotebookSetup {
    let setup = read(file).remove(folder.unwrap_or(ROOT_KEY)).unwrap_or_default();
    with_current_dark_paper(setup)
}

/// The dark paper before it became StarNote's (Dracula's background and its comment color).
const OLD_DARK_PAPER: (u32, u32) = (0xFF28_2A36, 0xFF62_72A4);

/// `setup`, with the old dark paper, remembered before it changed, as the current one.
fn with_current_dark_paper(setup: NotebookSetup) -> NotebookSetup {
    if (setup.background_argb, setup.pattern_argb) == OLD_DARK_PAPER {
        NotebookSetup { background_argb: DARK_PAPER, pattern_argb: DARK_LINES, ..setup }
    } else {
        setup
    }
}

/// Remembers `setup` for `folder` in `file`, replacing the file at once.
pub fn remember_defaults(
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
