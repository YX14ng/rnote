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

#[test]
fn add_reading_keeps_the_language_and_the_other_readings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");

    crate::ink_index::store::add_reading(
        &path,
        "es",
        "a",
        [1.0, 2.0, 3.0, 4.0],
        &["uno".to_owned()],
    )
    .unwrap();
    crate::ink_index::store::add_reading(&path, "es", "b", [0.0; 4], &["dos".to_owned()]).unwrap();

    let index = InkIndex::load(&path, "es");
    assert_eq!(index.entries.len(), 2);
    assert_eq!(index.entries["a"].rect, [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn a_late_reading_in_the_old_language_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    let add = |language: &str, hash: &str, text: &str| {
        crate::ink_index::store::add_reading(&path, language, hash, [0.0; 4], &[text.to_owned()])
            .unwrap();
    };
    add("es", "a", "hola");
    // The language changed to English: listing what to read empties the index into
    // English, and the first fragment is read again in it.
    crate::ink_index::store::with_index(&path, "en-US", |_| ()).unwrap();
    add("en-US", "a", "hello");

    // A reading asked for in Spanish before the change comes back now.
    add("es", "b", "tarde");

    let index = InkIndex::load(&path, "en-US");
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries["a"].candidates, ["hello"]);
}

#[test]
fn a_batch_of_readings_is_kept_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    // As the app sends them.
    let readings: Vec<crate::ink_index::store::Reading> = serde_json::from_str(
        r#"[{"hash":"a","rect":[1,2,3,4],"candidates":["uno","una","uña","ufa"]},
            {"hash":"b","rect":[0,0,0,0],"candidates":["dos"]}]"#,
    )
    .unwrap();

    crate::ink_index::store::add_readings(&path, "es", &readings).unwrap();

    let index = InkIndex::load(&path, "es");
    assert_eq!(index.entries.len(), 2);
    assert_eq!(index.entries["a"].rect, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(index.entries["a"].candidates, ["uno", "una", "uña"]);
}

#[test]
fn an_index_that_didnt_change_isnt_written() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.json");
    crate::ink_index::store::add_reading(&path, "es", "a", [0.0; 4], &["uno".to_owned()]).unwrap();
    // Writing would fail now.
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

    let result = crate::ink_index::store::with_index(&path, "es", |_| ());

    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    result.unwrap();
}
