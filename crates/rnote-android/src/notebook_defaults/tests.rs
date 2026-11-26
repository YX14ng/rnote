#![allow(clippy::unwrap_used)]

use super::{defaults_for, remember};
use crate::notebook_setup::{NotebookSetup, SetupLayout};

#[test]
fn defaults_are_remembered_per_folder() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notebook_defaults.json");
    let algebra = NotebookSetup { layout: SetupLayout::Infinite, ..NotebookSetup::default() };
    let root = NotebookSetup { cover: Some(0xFF3D_6FB6), ..NotebookSetup::default() };

    remember(&file, Some("Álgebra"), &algebra).unwrap();
    remember(&file, None, &root).unwrap();

    assert_eq!(defaults_for(&file, Some("Álgebra")), algebra);
    assert_eq!(defaults_for(&file, None), root);
    assert_eq!(defaults_for(&file, Some("Física")), NotebookSetup::default());
}

#[test]
fn missing_or_broken_defaults_file_gives_the_default() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notebook_defaults.json");
    assert_eq!(defaults_for(&file, None), NotebookSetup::default());

    std::fs::write(&file, b"{nope").unwrap();

    assert_eq!(defaults_for(&file, None), NotebookSetup::default());
    remember(&file, None, &NotebookSetup::default()).unwrap();
}

#[test]
fn the_old_dark_paper_is_remembered_as_the_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notebook_defaults.json");
    let old_dark = NotebookSetup {
        background_argb: 0xFF28_2A36,
        pattern_argb: 0xFF62_72A4,
        pattern_size_mm: 6.0,
        ..NotebookSetup::default()
    };
    remember(&file, Some("Algebra"), &old_dark).unwrap();

    let now = defaults_for(&file, Some("Algebra"));

    assert_eq!((now.background_argb, now.pattern_argb), (0xFF00_0000, 0xFF40_4040));
    // Everything else stays as it was chosen.
    assert_eq!(now.pattern_size_mm, 6.0);
}
