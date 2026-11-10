use std::fs;

use serde_json::Value;

use super::{library_at, paths, write};

fn library_json(root: &std::path::Path) -> Value {
    serde_json::from_slice(&fs::read(root.join(".rnote-library/library.json")).unwrap()).unwrap()
}

#[test]
fn delete_tag_leaves_a_tombstone_and_hides_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut library = library_at(dir.path());
    let id = library.create_tag("Parcial", 7).unwrap();

    library.delete_tag(&id).unwrap();

    assert!(library.tags().is_empty());
    let json = library_json(dir.path());
    assert_eq!(json["tags"][&id]["deleted"], true);
    assert_eq!(json["tags"][&id]["updated_at"], 1_000);
}

#[test]
fn deleted_tags_disappear_from_notebooks() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    let mut library = library_at(dir.path());
    let id = library.create_tag("t", 1).unwrap();
    library.set_tags("a.rnote", std::slice::from_ref(&id)).unwrap();

    library.delete_tag(&id).unwrap();

    assert!(library.notebooks().unwrap()[0].tags.is_empty());
}

#[test]
fn update_tag_changes_name_and_color() {
    let dir = tempfile::tempdir().unwrap();
    let mut library = library_at(dir.path());
    let id = library.create_tag("a", 1).unwrap();

    library.update_tag(&id, "b", 2).unwrap();

    let tags = library.tags();
    assert_eq!((tags[0].name.as_str(), tags[0].color), ("b", 2));
}

#[test]
fn metadata_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    let id = {
        let mut library = library_at(dir.path());
        let id = library.create_tag("t", 1).unwrap();
        library.set_tags("a.rnote", std::slice::from_ref(&id)).unwrap();
        library.trash("a.rnote").unwrap();
        id
    };

    let library = library_at(dir.path());

    assert_eq!(library.tags()[0].id, id);
    assert_eq!(library.trash_entries().len(), 1);
}

#[test]
fn corrupt_library_json_is_set_aside() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    write(dir.path(), ".rnote-library/library.json", b"{nope");

    let library = library_at(dir.path());

    assert!(dir.path().join(".rnote-library/library.json.bad").exists());
    assert!(library.tags().is_empty());
    assert_eq!(paths(&library), vec!["a.rnote"]);
}

#[test]
fn future_version_is_set_aside() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), ".rnote-library/library.json", br#"{"version": 99}"#);

    let library = library_at(dir.path());

    assert!(dir.path().join(".rnote-library/library.json.bad").exists());
    assert!(library.tags().is_empty());
}

#[test]
fn json_round_trip_matches_the_v1_format() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rnote", b"a");
    write(dir.path(), "b.rnote", b"b");
    let mut library = library_at(dir.path());
    let id = library.create_tag("Parcial", 0xFFEE_2233).unwrap();
    library.set_tags("a.rnote", std::slice::from_ref(&id)).unwrap();
    library.trash("b.rnote").unwrap();
    let trash_id = library.trash_entries()[0].id.clone();

    let json = library_json(dir.path());

    assert_eq!(json["version"], 1);
    assert_eq!(id.len(), 16);
    assert_eq!(json["tags"][&id]["name"], "Parcial");
    assert_eq!(json["tags"][&id]["color"], 0xFFEE_2233_u32);
    assert_eq!(json["tags"][&id]["deleted"], false);
    assert_eq!(json["notebook_tags"]["a.rnote"]["tags"][0], id.as_str());
    assert_eq!(json["notebook_tags"]["a.rnote"]["updated_at"], 1_000);
    assert_eq!(trash_id, "1000-b.rnote");
    assert_eq!(json["trash"][&trash_id]["original_path"], "b.rnote");
    assert_eq!(json["trash"][&trash_id]["deleted_at"], 1_000);
}
