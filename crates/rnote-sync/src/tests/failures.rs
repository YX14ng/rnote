//! What went wrong in a sync, so the apps can say it in the user's language.

use std::io;

use anyhow::anyhow;

use super::stub_http::Stub;
use crate::{DriveClient, FailureKind, Remote, RemoteError, failure_kind};

const JSON: &str = "application/json";

fn refusal(reason: &str) -> anyhow::Error {
    let body = serde_json::to_vec(&serde_json::json!({"error": {"errors": [{"reason": reason}]}}))
        .unwrap();
    let stub = Stub::start(vec![(403, JSON, body)]);
    let mut client = DriveClient::new(|| Ok("t".to_owned())).with_base_url(stub.url());
    match client.trash("n") {
        Err(RemoteError::Failed(error)) => error,
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn a_full_drive_is_told_apart() {
    assert_eq!(failure_kind(&refusal("storageQuotaExceeded")), FailureKind::DriveFull);
}

#[test]
fn other_refusals_of_drive_are_refusals() {
    assert_eq!(failure_kind(&refusal("insufficientFilePermissions")), FailureKind::DriveRefused);
}

#[test]
fn files_this_device_can_not_read_or_write_are_local() {
    let error = anyhow::Error::new(io::Error::from(io::ErrorKind::PermissionDenied))
        .context("Writing a.rnote");

    assert_eq!(failure_kind(&error), FailureKind::LocalFiles);
}

#[test]
fn a_file_changed_meanwhile_is_told_apart() {
    let error = anyhow::Error::new(crate::ChangedDuringSync("a.rnote".to_owned()));

    assert_eq!(failure_kind(&error), FailureKind::ChangedMeanwhile);
}

#[test]
fn anything_else_is_unexpected() {
    assert_eq!(failure_kind(&anyhow!("strange")), FailureKind::Unexpected);
}
