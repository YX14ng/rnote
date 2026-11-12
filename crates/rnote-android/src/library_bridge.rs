//! The library, for the app: requests and answers as JSON, so a single JNI entry point
//! serves every operation.

use std::path::Path;

use rnote_library::{Library, thumbnail_key};
use serde::Deserialize;
use serde_json::{Value, json};

/// An operation on the library; `op` names it, in snake case.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum LibraryRequest {
    List,
    CreateFolder { name: String },
    RenameFolder { folder: String, new_name: String },
    DeleteFolder { folder: String },
    Rename { path: String, new_name: String },
    MoveTo { path: String, folder: Option<String> },
    Duplicate { path: String },
    NewNotebookPath { folder: Option<String>, stem: String },
    Trash { path: String },
    Restore { id: String },
    DeleteForever { id: String },
    EmptyTrash,
    CreateTag { name: String, color: u32 },
    UpdateTag { id: String, name: String, color: u32 },
    DeleteTag { id: String },
    SetTags { path: String, tags: Vec<String> },
    SetCover { path: String, color: Option<u32> },
    ImportXopp { path: String },
    UniqueName { folder: Option<String>, file_name: String },
}

// Only the JNI entry points, built for Android, call it outside of tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
/// Runs the JSON `request` on the library at `root` and answers in JSON: the library's
/// contents for `list`, `{"path": ...}` or `{"id": ...}` for what creates or moves
/// something, `{}` otherwise.
pub(crate) fn handle_request(root: &Path, request: &str) -> anyhow::Result<String> {
    let request: LibraryRequest = serde_json::from_str(request)?;
    let mut library = Library::open(root)?;
    let path = |path: String| json!({ "path": path });
    let answer = match request {
        LibraryRequest::List => list(&library)?,
        LibraryRequest::CreateFolder { name } => json!({ "name": library.create_folder(&name)? }),
        LibraryRequest::RenameFolder { folder, new_name } => {
            json!({ "name": library.rename_folder(&folder, &new_name)? })
        }
        LibraryRequest::DeleteFolder { folder } => {
            library.delete_folder(&folder)?;
            json!({})
        }
        LibraryRequest::Rename { path: from, new_name } => path(library.rename(&from, &new_name)?),
        LibraryRequest::MoveTo { path: from, folder } => {
            path(library.move_to(&from, folder.as_deref())?)
        }
        LibraryRequest::Duplicate { path: from } => path(library.duplicate(&from)?),
        LibraryRequest::NewNotebookPath { folder, stem } => {
            path(library.new_notebook_path(folder.as_deref(), &stem)?)
        }
        LibraryRequest::Trash { path } => {
            library.trash(&path)?;
            json!({})
        }
        LibraryRequest::Restore { id } => path(library.restore(&id)?),
        LibraryRequest::DeleteForever { id } => {
            library.delete_forever(&id)?;
            json!({})
        }
        LibraryRequest::EmptyTrash => {
            library.empty_trash()?;
            json!({})
        }
        LibraryRequest::CreateTag { name, color } => {
            json!({ "id": library.create_tag(&name, color)? })
        }
        LibraryRequest::UpdateTag { id, name, color } => {
            library.update_tag(&id, &name, color)?;
            json!({})
        }
        LibraryRequest::DeleteTag { id } => {
            library.delete_tag(&id)?;
            json!({})
        }
        LibraryRequest::SetTags { path, tags } => {
            library.set_tags(&path, &tags)?;
            json!({})
        }
        LibraryRequest::SetCover { path, color } => {
            library.set_cover(&path, color)?;
            json!({})
        }
        LibraryRequest::ImportXopp { path: from } => path(library.import_xopp(&from)?),
        LibraryRequest::UniqueName { folder, file_name } => {
            path(library.free_path(folder.as_deref(), &file_name)?)
        }
    };
    Ok(answer.to_string())
}

/// Everything the library screen shows; each notebook also carries its thumbnail key.
fn list(library: &Library) -> anyhow::Result<Value> {
    let notebooks = library
        .notebooks()?
        .into_iter()
        .map(|notebook| {
            let key = thumbnail_key(&notebook);
            let mut value = serde_json::to_value(notebook)?;
            value["key"] = Value::String(key);
            Ok(value)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(json!({
        "notebooks": notebooks,
        "folders": library.folders()?,
        "tags": library.tags(),
        "trash": library.trash_entries(),
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::fs;

    use serde_json::Value;

    use super::handle_request;

    fn call(root: &std::path::Path, request: &str) -> Value {
        serde_json::from_str(&handle_request(root, request).unwrap()).unwrap()
    }

    #[test]
    fn list_returns_the_four_sections() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rnote"), b"a").unwrap();

        let answer = call(dir.path(), r#"{"op": "list"}"#);

        assert_eq!(answer["notebooks"][0]["path"], "a.rnote");
        assert_eq!(answer["notebooks"][0]["kind"], "rnote");
        assert_eq!(answer["notebooks"][0]["key"].as_str().unwrap().len(), 16);
        assert_eq!(answer["folders"], Value::Array(Vec::new()));
        assert_eq!(answer["tags"], Value::Array(Vec::new()));
        assert_eq!(answer["trash"], Value::Array(Vec::new()));
    }

    #[test]
    fn rename_request_returns_the_new_path() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rnote"), b"a").unwrap();

        let answer = call(dir.path(), r#"{"op": "rename", "path": "a.rnote", "new_name": "b"}"#);

        assert_eq!(answer["path"], "b.rnote");
        assert!(dir.path().join("b.rnote").exists());
    }

    #[test]
    fn create_tag_request_returns_the_id() {
        let dir = tempfile::tempdir().unwrap();

        let answer = call(dir.path(), r#"{"op": "create_tag", "name": "t", "color": 4294901760}"#);

        assert_eq!(answer["id"].as_str().unwrap().len(), 16);
    }

    #[test]
    fn unique_name_request_avoids_taken_names() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rnote"), b"a").unwrap();

        let answer =
            call(dir.path(), r#"{"op": "unique_name", "folder": null, "file_name": "a.rnote"}"#);

        assert_eq!(answer["path"], "a (2).rnote");
    }

    #[test]
    fn set_cover_request_sets_the_cover() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rnote"), b"a").unwrap();

        call(dir.path(), r#"{"op": "set_cover", "path": "a.rnote", "color": 4282216374}"#);
        assert_eq!(
            call(dir.path(), r#"{"op": "list"}"#)["notebooks"][0]["cover"],
            4_282_216_374_u32
        );

        call(dir.path(), r#"{"op": "set_cover", "path": "a.rnote", "color": null}"#);
        assert_eq!(call(dir.path(), r#"{"op": "list"}"#)["notebooks"][0]["cover"], Value::Null);
    }

    #[test]
    fn unknown_op_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(handle_request(dir.path(), r#"{"op": "format_disk"}"#).is_err());
    }

    #[test]
    fn bad_json_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(handle_request(dir.path(), "{nope").is_err());
    }
}
