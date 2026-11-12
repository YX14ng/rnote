use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::stub_http::{Response, Stub};
use crate::{DriveClient, Remote, RemoteError, RemoteFile};

const JSON: &str = "application/json";
const FOLDER: &str = "application/vnd.google-apps.folder";

type Sleeps = Rc<RefCell<Vec<Duration>>>;

fn client(stub: &Stub) -> (DriveClient, Sleeps) {
    let sleeps: Sleeps = Rc::default();
    let recorded = Rc::clone(&sleeps);
    let client = DriveClient::new(|| Ok("token-1".to_owned()))
        .with_base_url(stub.url())
        .with_sleep(move |duration| recorded.borrow_mut().push(duration));
    (client, sleeps)
}

fn json(value: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&value).unwrap()
}

fn file_json(id: &str, name: &str, parent: &str) -> serde_json::Value {
    serde_json::json!({"id": id, "name": name, "mimeType": "application/octet-stream",
        "parents": [parent], "md5Checksum": format!("md5-{id}"), "modifiedTime": "t"})
}

fn remote_file(id: &str, name: &str, parent: &str) -> RemoteFile {
    RemoteFile {
        id: id.to_owned(),
        parent: parent.to_owned(),
        name: name.to_owned(),
        md5: format!("md5-{id}"),
        modified: "t".to_owned(),
    }
}

fn seconds(list: &[u64]) -> Vec<Duration> {
    list.iter().map(|&s| Duration::from_secs(s)).collect()
}

#[test]
fn list_follows_page_tokens_and_splits_files_and_folders() {
    let first = serde_json::json!({"nextPageToken": "p2", "files": [
        {"id": "r", "name": "Rnote", "mimeType": FOLDER, "parents": ["top"]},
        file_json("a", "a.rnote", "r"),
    ]});
    let second = serde_json::json!({"files": [file_json("b", "b.rnote", "r")]});
    let stub = Stub::start(vec![(200, JSON, json(first)), (200, JSON, json(second))]);
    let (mut client, _) = client(&stub);

    let listing = client.list().unwrap();

    assert_eq!(listing.folders.len(), 1);
    assert_eq!(listing.folders[0].parent.as_deref(), Some("top"));
    assert_eq!(
        listing.files,
        vec![remote_file("a", "a.rnote", "r"), remote_file("b", "b.rnote", "r")]
    );
    let requests = stub.requests();
    assert_eq!(requests[0].path(), "/drive/v3/files");
    assert_eq!(requests[0].query("q").as_deref(), Some("trashed = false"));
    assert_eq!(requests[0].query("pageSize").as_deref(), Some("1000"));
    assert_eq!(requests[0].query("pageToken"), None);
    assert_eq!(requests[1].query("pageToken").as_deref(), Some("p2"));
}

#[test]
fn find_root_asks_for_the_rnote_folder_at_the_top() {
    let stub = Stub::start(vec![
        (200, JSON, json(serde_json::json!({"files": [{"id": "r"}]}))),
        (200, JSON, json(serde_json::json!({"files": []}))),
    ]);
    let (mut client, _) = client(&stub);

    assert_eq!(client.find_root().unwrap().as_deref(), Some("r"));
    assert_eq!(client.find_root().unwrap(), None);
    let q = stub.requests()[0].query("q").unwrap();
    assert!(q.contains("name = 'Rnote'"), "{q}");
    assert!(q.contains("'root' in parents") && q.contains("trashed = false"), "{q}");
    assert!(q.contains(FOLDER), "{q}");
}

#[test]
fn download_asks_for_the_media() {
    let stub = Stub::start(vec![(200, "application/octet-stream", b"\x00\x01data".to_vec())]);
    let (mut client, _) = client(&stub);

    assert_eq!(client.download("n").unwrap(), b"\x00\x01data");
    let request = &stub.requests()[0];
    assert_eq!((request.method.as_str(), request.path()), ("GET", "/drive/v3/files/n"));
    assert_eq!(request.query("alt").as_deref(), Some("media"));
}

#[test]
fn create_file_sends_multipart_with_name_and_parent() {
    let stub = Stub::start(vec![(200, JSON, json(file_json("n", "a.rnote", "r")))]);
    let (mut client, _) = client(&stub);

    let file = client.create_file("r", "a.rnote", b"\x00rnote-bytes").unwrap();

    assert_eq!(file, remote_file("n", "a.rnote", "r"));
    let request = &stub.requests()[0];
    assert_eq!((request.method.as_str(), request.path()), ("POST", "/upload/drive/v3/files"));
    assert_eq!(request.query("uploadType").as_deref(), Some("multipart"));
    assert!(request.header("content-type").unwrap().starts_with("multipart/related; boundary="));
    let body = String::from_utf8_lossy(&request.body);
    assert!(body.contains(r#""name":"a.rnote""#), "{body}");
    assert!(body.contains(r#""parents":["r"]"#), "{body}");
    assert!(request.body.windows(13).any(|window| window == b"\x00rnote-bytes\r"));
}

#[test]
fn update_file_sends_the_bytes() {
    let stub = Stub::start(vec![(200, JSON, json(file_json("n", "a.rnote", "r")))]);
    let (mut client, _) = client(&stub);

    client.update_file("n", b"new").unwrap();

    let request = &stub.requests()[0];
    assert_eq!((request.method.as_str(), request.path()), ("PATCH", "/upload/drive/v3/files/n"));
    assert_eq!(request.query("uploadType").as_deref(), Some("media"));
    assert_eq!(request.body, b"new");
}

const FIVE_MIB: usize = 5 * 1024 * 1024;

/// The answers of a resumable upload: the session, then the uploaded file.
fn resumable_answers(file: serde_json::Value) -> Vec<Response> {
    vec![
        (200, JSON, vec![("Location", "{url}/upload/session-1?upload_id=u1")], Vec::new()),
        (200, JSON, Vec::new(), json(file)),
    ]
}

#[test]
fn upload_of_exactly_5_mib_is_multipart() {
    let stub = Stub::start(vec![(200, JSON, json(file_json("n", "a.rnote", "r")))]);
    let (mut client, _) = client(&stub);

    client.create_file("r", "a.rnote", &vec![0; FIVE_MIB]).unwrap();

    let requests = stub.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].query("uploadType").as_deref(), Some("multipart"));
}

#[test]
fn big_create_uses_a_resumable_session() {
    let stub = Stub::start_with_headers(resumable_answers(file_json("n", "a.rnote", "r")));
    let (mut client, _) = client(&stub);
    let bytes = vec![7; FIVE_MIB + 1];

    let file = client.create_file("r", "a.rnote", &bytes).unwrap();

    assert_eq!(file, remote_file("n", "a.rnote", "r"));
    let requests = stub.requests();
    assert_eq!(requests.len(), 2);
    let start = &requests[0];
    assert_eq!((start.method.as_str(), start.path()), ("POST", "/upload/drive/v3/files"));
    assert_eq!(start.query("uploadType").as_deref(), Some("resumable"));
    assert_eq!(start.json(), serde_json::json!({"name": "a.rnote", "parents": ["r"]}));
    let upload = &requests[1];
    assert_eq!((upload.method.as_str(), upload.path()), ("PUT", "/upload/session-1"));
    assert_eq!(upload.query("upload_id").as_deref(), Some("u1"));
    assert_eq!(upload.header("authorization"), Some("Bearer token-1"));
    assert!(upload.body == bytes, "the upload carries the file");
}

#[test]
fn big_update_uses_a_resumable_session() {
    let stub = Stub::start_with_headers(resumable_answers(file_json("n", "a.rnote", "r")));
    let (mut client, _) = client(&stub);
    let bytes = vec![9; FIVE_MIB + 1];

    client.update_file("n", &bytes).unwrap();

    let requests = stub.requests();
    let start = &requests[0];
    assert_eq!((start.method.as_str(), start.path()), ("PATCH", "/upload/drive/v3/files/n"));
    assert_eq!(start.query("uploadType").as_deref(), Some("resumable"));
    assert_eq!((requests[1].method.as_str(), requests[1].path()), ("PUT", "/upload/session-1"));
    assert!(requests[1].body == bytes, "the upload carries the file");
}

#[test]
fn resumable_upload_failure_is_an_error() {
    let mut answers = resumable_answers(file_json("n", "a.rnote", "r"));
    answers[1] = (400, JSON, Vec::new(), b"{}".to_vec());
    let stub = Stub::start_with_headers(answers);
    let (mut client, _) = client(&stub);

    let result = client.create_file("r", "a.rnote", &vec![0; FIVE_MIB + 1]);

    assert!(matches!(result, Err(RemoteError::Failed(_))), "{result:?}");
    let requests = stub.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].method, "PUT");
}

#[test]
fn resumable_start_without_location_is_an_error() {
    let stub = Stub::start(vec![(200, JSON, Vec::new())]);
    let (mut client, _) = client(&stub);

    let result = client.update_file("n", &vec![0; FIVE_MIB + 1]);

    assert!(matches!(result, Err(RemoteError::Failed(_))), "{result:?}");
    let requests = stub.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].query("uploadType").as_deref(), Some("resumable"));
}

#[test]
fn move_file_sends_add_and_remove_parents_and_name() {
    let stub = Stub::start(vec![(200, JSON, json(file_json("n", "b.rnote", "f2")))]);
    let (mut client, _) = client(&stub);

    let file = client.move_file("n", "f1", "f2", "b.rnote").unwrap();

    assert_eq!(file, remote_file("n", "b.rnote", "f2"));
    let request = &stub.requests()[0];
    assert_eq!((request.method.as_str(), request.path()), ("PATCH", "/drive/v3/files/n"));
    assert_eq!(request.query("addParents").as_deref(), Some("f2"));
    assert_eq!(request.query("removeParents").as_deref(), Some("f1"));
    assert_eq!(request.json(), serde_json::json!({"name": "b.rnote"}));
}

#[test]
fn create_folder_sends_the_folder_type() {
    let stub = Stub::start(vec![
        (200, JSON, json(serde_json::json!({"id": "f"}))),
        (200, JSON, json(serde_json::json!({"id": "r"}))),
    ]);
    let (mut client, _) = client(&stub);

    assert_eq!(client.create_folder(Some("r"), "Orga").unwrap(), "f");
    assert_eq!(client.create_folder(None, "Rnote").unwrap(), "r");
    let requests = stub.requests();
    assert_eq!((requests[0].method.as_str(), requests[0].path()), ("POST", "/drive/v3/files"));
    assert_eq!(
        requests[0].json(),
        serde_json::json!({"name": "Orga", "mimeType": FOLDER, "parents": ["r"]})
    );
    assert_eq!(requests[1].json(), serde_json::json!({"name": "Rnote", "mimeType": FOLDER}));
}

#[test]
fn trash_never_deletes() {
    let stub = Stub::start(vec![(200, JSON, json(serde_json::json!({"id": "n"})))]);
    let (mut client, _) = client(&stub);

    client.trash("n").unwrap();

    let request = &stub.requests()[0];
    assert_eq!((request.method.as_str(), request.path()), ("PATCH", "/drive/v3/files/n"));
    assert_eq!(request.json(), serde_json::json!({"trashed": true}));
}

#[test]
fn every_request_carries_the_bearer_token() {
    let stub = Stub::start(vec![
        (200, JSON, json(serde_json::json!({"files": []}))),
        (200, JSON, json(serde_json::json!({"id": "n"}))),
    ]);
    let (mut client, _) = client(&stub);

    client.list().unwrap();
    client.trash("n").unwrap();

    for request in stub.requests() {
        assert_eq!(request.header("authorization"), Some("Bearer token-1"));
    }
}

#[test]
fn unauthorized_is_401() {
    let stub = Stub::start(vec![(401, JSON, json(serde_json::json!({"error": {}})))]);
    let (mut client, sleeps) = client(&stub);

    assert!(matches!(client.list(), Err(RemoteError::Unauthorized)));
    assert!(sleeps.borrow().is_empty());
}

#[test]
fn retries_429_then_succeeds() {
    let empty = json(serde_json::json!({"files": []}));
    let stub = Stub::start(vec![
        (429, JSON, b"{}".to_vec()),
        (429, JSON, b"{}".to_vec()),
        (200, JSON, empty),
    ]);
    let (mut client, sleeps) = client(&stub);

    assert!(client.list().is_ok());
    assert_eq!(*sleeps.borrow(), seconds(&[1, 2]));
}

#[test]
fn rate_limit_403_is_retried_but_other_403_is_not() {
    let limited = json(serde_json::json!({"error": {"errors": [{"reason": "rateLimitExceeded"}]}}));
    let denied =
        json(serde_json::json!({"error": {"errors": [{"reason": "insufficientFilePermissions"}]}}));
    let stub = Stub::start(vec![
        (403, JSON, limited),
        (200, JSON, json(serde_json::json!({"id": "n"}))),
        (403, JSON, denied),
    ]);
    let (mut client, sleeps) = client(&stub);

    assert!(client.trash("n").is_ok());
    assert!(matches!(client.trash("n"), Err(RemoteError::Failed(_))));
    assert_eq!(*sleeps.borrow(), seconds(&[1]));
}

#[test]
fn gives_up_after_five_retries() {
    let stub = Stub::start((0..6).map(|_| (503, JSON, b"{}".to_vec())).collect());
    let (mut client, sleeps) = client(&stub);

    assert!(matches!(client.list(), Err(RemoteError::Failed(_))));
    assert_eq!(*sleeps.borrow(), seconds(&[1, 2, 4, 8, 16]));
}

#[test]
fn refused_connection_is_offline() {
    let mut client =
        DriveClient::new(|| Ok("token".to_owned())).with_base_url("http://127.0.0.1:1");

    assert!(matches!(client.list(), Err(RemoteError::Offline(_))));
}

#[test]
fn a_failing_token_source_stops_the_request() {
    let mut client =
        DriveClient::new(|| Err(RemoteError::Unauthorized)).with_base_url("http://127.0.0.1:1");

    assert!(matches!(client.list(), Err(RemoteError::Unauthorized)));
}

#[test]
fn get_returns_the_file_or_none_when_trashed_or_missing() {
    let mut trashed = file_json("n", "a.rnote", "r");
    trashed["trashed"] = serde_json::json!(true);
    let stub = Stub::start(vec![
        (200, JSON, json(file_json("n", "a.rnote", "r"))),
        (200, JSON, json(trashed)),
        (404, JSON, json(serde_json::json!({"error": {"code": 404}}))),
    ]);
    let (mut client, sleeps) = client(&stub);

    assert_eq!(client.get("n").unwrap(), Some(remote_file("n", "a.rnote", "r")));
    assert_eq!(client.get("n").unwrap(), None);
    assert_eq!(client.get("n").unwrap(), None);
    let request = &stub.requests()[0];
    assert_eq!((request.method.as_str(), request.path()), ("GET", "/drive/v3/files/n"));
    assert!(request.query("fields").unwrap().contains("trashed"));
    assert!(sleeps.borrow().is_empty());
}

#[test]
fn revisions_follow_page_tokens_and_download_one() {
    let first = serde_json::json!({"nextPageToken": "p2",
        "revisions": [{"id": "r1", "md5Checksum": "m1"}]});
    let second = serde_json::json!({"revisions": [{"id": "r2", "md5Checksum": "m2"}]});
    let stub = Stub::start(vec![
        (200, JSON, json(first)),
        (200, JSON, json(second)),
        (200, "application/octet-stream", b"old".to_vec()),
    ]);
    let (mut client, _) = client(&stub);

    let revisions = client.revisions("n").unwrap();
    let bytes = client.download_revision("n", "r1").unwrap();

    let pairs: Vec<(&str, &str)> =
        revisions.iter().map(|r| (r.id.as_str(), r.md5.as_str())).collect();
    assert_eq!(pairs, vec![("r1", "m1"), ("r2", "m2")]);
    assert_eq!(bytes, b"old");
    let requests = stub.requests();
    assert_eq!(requests[0].path(), "/drive/v3/files/n/revisions");
    assert_eq!(requests[1].query("pageToken").as_deref(), Some("p2"));
    assert_eq!(requests[2].path(), "/drive/v3/files/n/revisions/r1");
    assert_eq!(requests[2].query("alt").as_deref(), Some("media"));
}

#[test]
fn list_reads_when_folders_were_created() {
    let page = serde_json::json!({"files": [{"id": "r", "name": "Rnote", "mimeType": FOLDER,
        "parents": ["top"], "createdTime": "2026-11-10T10:00:00.000Z"}]});
    let stub = Stub::start(vec![(200, JSON, json(page))]);
    let (mut client, _) = client(&stub);

    let listing = client.list().unwrap();

    assert_eq!(listing.folders[0].created, "2026-11-10T10:00:00.000Z");
    assert!(stub.requests()[0].query("fields").unwrap().contains("createdTime"));
}
