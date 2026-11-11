//! Revoking the access token, and naming conflict copies from outside the sync.

use std::fs;

use chrono::NaiveDate;

use super::stub_http::Stub;
use crate::{RemoteError, conflict_copy_path, revoke_at};

const JSON: &str = "application/json";

#[test]
fn revoke_posts_the_token_as_a_form() {
    let stub = Stub::start(vec![(200, JSON, b"{}".to_vec())]);

    revoke_at(stub.url(), "token-1").unwrap();

    let request = &stub.requests()[0];
    assert_eq!((request.method.as_str(), request.path()), ("POST", "/revoke"));
    assert_eq!(request.header("content-type"), Some("application/x-www-form-urlencoded"));
    assert_eq!(request.body, b"token=token-1");
}

#[test]
fn revoke_of_an_invalid_token_is_ok() {
    let stub = Stub::start(vec![(400, JSON, br#"{"error": "invalid_token"}"#.to_vec())]);

    revoke_at(stub.url(), "token-1").unwrap();
}

#[test]
fn revoke_server_error_fails() {
    let stub = Stub::start(vec![(500, JSON, b"{}".to_vec())]);

    assert!(matches!(revoke_at(stub.url(), "token-1"), Err(RemoteError::Failed(_))));
}

#[test]
fn revoke_unreachable_is_offline() {
    assert!(matches!(revoke_at("http://127.0.0.1:9", "token-1"), Err(RemoteError::Offline(_))));
}

#[test]
fn conflict_copy_path_skips_taken_names() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("Orga")).unwrap();
    fs::write(dir.path().join("a.rnote"), b"a").unwrap();
    fs::write(dir.path().join("a (conflicto, Tab, 2026-11-11 10.00).rnote"), b"b").unwrap();
    let now = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap().and_hms_opt(10, 0, 0).unwrap();

    assert_eq!(
        conflict_copy_path(dir.path(), "a.rnote", "Tab", now),
        "a (conflicto, Tab, 2026-11-11 10.00) (2).rnote"
    );
    assert_eq!(
        conflict_copy_path(dir.path(), "Orga/a.rnote", "Tab", now),
        "Orga/a (conflicto, Tab, 2026-11-11 10.00).rnote"
    );
}
