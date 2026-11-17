//! The local index of what the recognizer read.

use crate::ink_index::fragments::{InkStroke, fragments};
use crate::ink_index::store::{Entry, InkIndex, pending};

fn word(x: f64) -> InkStroke {
    InkStroke { points: vec![(x, 0.0), (x + 20.0, 20.0), (x + 40.0, 0.0)] }
}

fn entry(text: &str) -> Entry {
    Entry { rect: [0.0; 4], candidates: vec![text.to_owned()] }
}

#[test]
fn pending_drops_fragments_that_are_gone() {
    let mut index = InkIndex::empty("es");
    index.entries.insert("gone".to_owned(), entry("viejo"));
    let current = fragments(&[word(0.0)]);

    let todo = pending(&mut index, &current);

    assert_eq!(todo.len(), 1);
    assert!(index.entries.is_empty());
}

#[test]
fn unchanged_fragments_keep_their_candidates() {
    let current = fragments(&[word(0.0), word(200.0)]);
    let mut index = InkIndex::empty("es");
    index.entries.insert(current[0].hash.clone(), entry("hola"));

    let todo = pending(&mut index, &current);

    assert_eq!(todo.len(), 1);
    assert_eq!(todo[0].hash, current[1].hash);
    assert_eq!(index.entries[&current[0].hash].candidates, ["hola"]);
}

#[test]
fn saved_index_loads_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ink").join("a.json");
    let mut index = InkIndex::empty("es");
    index.entries.insert("h".to_owned(), entry("hola"));

    index.save(&path).unwrap();
    let loaded = InkIndex::load(&path, "es");

    assert_eq!(loaded, index);
    assert_eq!(InkIndex::stored_language(&path).as_deref(), Some("es"));
}

#[test]
fn changing_the_language_empties_the_index() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    let mut index = InkIndex::empty("es");
    index.entries.insert("h".to_owned(), entry("hola"));
    index.save(&path).unwrap();

    let loaded = InkIndex::load(&path, "en-US");

    assert_eq!(loaded, InkIndex::empty("en-US"));
}

#[test]
fn unreadable_index_file_is_an_empty_index() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    std::fs::write(&path, b"{not json").unwrap();

    assert_eq!(InkIndex::load(&path, "es"), InkIndex::empty("es"));
    assert_eq!(InkIndex::load(&dir.path().join("missing.json"), "es"), InkIndex::empty("es"));
    assert_eq!(InkIndex::stored_language(&path), None);
}
