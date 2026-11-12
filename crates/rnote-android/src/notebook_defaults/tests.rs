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
