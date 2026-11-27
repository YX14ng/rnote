//! What went wrong in a sync, told apart so the apps can say it in the user's language
//! instead of showing the technical chain.

/// Drive turned a request down.
#[derive(Debug, thiserror::Error)]
#[error("Drive answered {status}: {body}")]
pub struct DriveRefusal {
    /// The HTTP status.
    pub status: u16,
    /// The reason Drive gave, e.g. `storageQuotaExceeded`.
    pub reason: Option<String>,
    body: String,
}

impl DriveRefusal {
    /// The refusal Drive answered with `status` and `body`.
    pub fn new(status: u16, body: &[u8]) -> Self {
        let reason = serde_json::from_slice::<serde_json::Value>(body)
            .ok()
            .and_then(|json| json.pointer("/error/errors/0/reason")?.as_str().map(str::to_owned));
        Self { status, reason, body: String::from_utf8_lossy(body).into_owned() }
    }
}

/// A file changed or went away, here or in Drive, while the sync was working on it; the
/// next sync takes it as it is then.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ChangedDuringSync(pub String);

/// What kind of failure stopped a sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// The Google account has no space left.
    DriveFull,
    /// Drive refused a request for another reason.
    DriveRefused,
    /// A file changed while the sync worked on it: trying again is enough.
    ChangedMeanwhile,
    /// Files on this device couldn't be read or written.
    LocalFiles,
    /// Anything else.
    Unexpected,
}

/// What kind of failure `error` is.
#[must_use]
pub fn failure_kind(error: &anyhow::Error) -> FailureKind {
    if let Some(refusal) = error.chain().find_map(|cause| cause.downcast_ref::<DriveRefusal>()) {
        return match refusal.reason.as_deref() {
            Some("storageQuotaExceeded" | "quotaExceeded") => FailureKind::DriveFull,
            _ => FailureKind::DriveRefused,
        };
    }
    if error.chain().any(|cause| cause.is::<ChangedDuringSync>()) {
        FailureKind::ChangedMeanwhile
    } else if error.chain().any(|cause| cause.is::<std::io::Error>()) {
        FailureKind::LocalFiles
    } else {
        FailureKind::Unexpected
    }
}
