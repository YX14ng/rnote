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

/// A `library.json` with `tags`, `notebook_tags` and `trash` given as JSON objects.
fn meta_json(tags: Value, notebook_tags: Value, trash: Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "tags": tags,
        "notebook_tags": notebook_tags,
        "trash": trash,
    }))
    .unwrap()
}

fn tag(name: &str, updated_at: i64, deleted: bool) -> Value {
    serde_json::json!({"name": name, "color": 1, "updated_at": updated_at, "deleted": deleted})
}

fn merged(local: &[u8], remote: &[u8]) -> Value {
    serde_json::from_slice(&crate::merge_meta(local, remote).unwrap()).unwrap()
}

#[test]
fn merge_keeps_entries_from_both_sides() {
    let empty = serde_json::json!({});
    let local =
        meta_json(serde_json::json!({"a": tag("A", 1, false)}), empty.clone(), empty.clone());
    let remote = meta_json(serde_json::json!({"b": tag("B", 1, false)}), empty.clone(), empty);

    let json = merged(&local, &remote);

    assert_eq!(json["tags"]["a"]["name"], "A");
    assert_eq!(json["tags"]["b"]["name"], "B");
}

#[test]
fn merge_takes_the_newer_record_of_each_entry() {
    let empty = serde_json::json!({});
    let local = meta_json(
        serde_json::json!({"a": tag("Viejo", 5, false)}),
        serde_json::json!({"x.rnote": {"tags": ["a"], "updated_at": 9, "deleted": false}}),
        empty.clone(),
    );
    let remote = meta_json(
        serde_json::json!({"a": tag("Nuevo", 9, false)}),
        serde_json::json!({"x.rnote": {"tags": [], "updated_at": 4, "deleted": false}}),
        empty,
    );

    let json = merged(&local, &remote);

    assert_eq!(json["tags"]["a"]["name"], "Nuevo");
    assert_eq!(json["notebook_tags"]["x.rnote"]["tags"], serde_json::json!(["a"]));
}

#[test]
fn merge_keeps_tombstones() {
    let empty = serde_json::json!({});
    let live = serde_json::json!({"x": {"original_path": "a.rnote", "deleted_at": 1,
        "updated_at": 5, "deleted": false}});
    let tombstone = serde_json::json!({"x": {"original_path": "a.rnote", "deleted_at": 1,
        "updated_at": 9, "deleted": true}});
    let local = meta_json(empty.clone(), empty.clone(), live);
    let remote = meta_json(empty.clone(), empty, tombstone);

    assert_eq!(merged(&local, &remote)["trash"]["x"]["deleted"], true);
}

#[test]
fn merge_is_commutative() {
    let empty = serde_json::json!({});
    let a = meta_json(serde_json::json!({"t": tag("Uno", 7, false)}), empty.clone(), empty.clone());
    let b = meta_json(serde_json::json!({"t": tag("Dos", 7, true)}), empty.clone(), empty.clone());
    let c = meta_json(serde_json::json!({"t": tag("Tres", 7, false)}), empty.clone(), empty);

    assert_eq!(crate::merge_meta(&a, &b).unwrap(), crate::merge_meta(&b, &a).unwrap());
    assert_eq!(crate::merge_meta(&a, &c).unwrap(), crate::merge_meta(&c, &a).unwrap());
    assert_eq!(merged(&a, &b)["tags"]["t"]["deleted"], true);
}

#[test]
fn merge_with_an_unreadable_side_keeps_the_readable_one() {
    let empty = serde_json::json!({});
    let good = meta_json(serde_json::json!({"a": tag("A", 1, false)}), empty.clone(), empty);
    let expected: Value = serde_json::from_slice(&good).unwrap();

    assert_eq!(merged(&good, b"{"), expected);
    assert_eq!(merged(b"{", &good), expected);
    assert_eq!(merged(&good, br#"{"version": 99}"#), expected);
}

#[test]
fn merge_of_two_unreadable_sides_is_an_error() {
    assert!(crate::merge_meta(b"{", b"nope").is_err());
}

#[test]
fn saving_the_metadata_waits_for_the_write_guard() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_owned();
    let meta = root.join(".rnote-library").join(crate::META_FILE);
    let held = crate::write_guard();

    let writer = std::thread::spawn(move || {
        crate::Library::open(&root).unwrap().create_tag("Parcial", 1).unwrap();
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    let written_while_held = meta.exists();
    drop(held);
    writer.join().unwrap();

    assert!(!written_while_held);
    assert!(meta.exists());
}

fn cover(color: u32, updated_at: i64, deleted: bool) -> Value {
    serde_json::json!({"color": color, "updated_at": updated_at, "deleted": deleted})
}

#[test]
fn merge_keeps_covers_from_a_copy_without_them() {
    // As the version before covers wrote it.
    let old = br#"{"version":1,"tags":{},"notebook_tags":{},"trash":{}}"#;
    let new = serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "notebook_covers": {"a.rnote": cover(7, 5, false)},
    }))
    .unwrap();

    for merged in [merged(old, &new), merged(&new, old)] {
        assert_eq!(merged["notebook_covers"]["a.rnote"]["color"], 7, "{merged}");
    }
}

#[test]
fn merge_picks_the_newer_cover() {
    let side = |record: Value| {
        serde_json::to_vec(&serde_json::json!({"version": 1, "notebook_covers": {"a": record}}))
            .unwrap()
    };

    let newer = merged(&side(cover(1, 1, false)), &side(cover(2, 2, false)));
    assert_eq!(newer["notebook_covers"]["a"]["color"], 2);
    let removed = merged(&side(cover(1, 3, true)), &side(cover(2, 2, false)));
    assert_eq!(removed["notebook_covers"]["a"]["deleted"], true);
}

#[test]
fn unknown_keys_survive_load_and_save() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), ".rnote-library/library.json", br#"{"version":1,"futuro":{"a":1}}"#);
    write(dir.path(), "a.rnote", b"a");
    let mut library = library_at(dir.path());

    library.create_tag("t", 1).unwrap();

    assert_eq!(library_json(dir.path())["futuro"], serde_json::json!({"a": 1}));
}

#[test]
fn merge_keeps_unknown_keys_of_both_sides() {
    let merged = merged(br#"{"version":1,"x":1}"#, br#"{"version":1,"x":2,"y":3}"#);

    assert_eq!((merged["x"].clone(), merged["y"].clone()), (1.into(), 3.into()));
}
