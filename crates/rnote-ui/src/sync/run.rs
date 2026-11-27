//! One sync, with one more try when Drive refuses the access token or a file changed while
//! it ran.

use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use rnote_sync::oauth::AccessTokens;
use rnote_sync::{SyncError, SyncReport};
use tracing::warn;

use super::credentials::RefreshTokenStore;
use super::status::SyncStatus;

/// Runs `attempt` and words how it went. When Drive refuses the access token, it's dropped
/// and `attempt` runs once more with a new one; refused again (e.g. the access was removed
/// from the Google account), the refresh token is forgotten and the user has to sign in,
/// unless the user signed in again meanwhile.
pub(crate) fn run_with_retry(
    tokens: &Arc<Mutex<AccessTokens>>,
    store: &dyn RefreshTokenStore,
    mut attempt: impl FnMut() -> Result<SyncReport, SyncError>,
) -> SyncStatus {
    // Signing out and in again during the sync replaces it; that one isn't to be forgotten.
    let token_at_start = store.load().ok().flatten();
    let mut outcome = attempt();
    if matches!(outcome, Err(SyncError::Unauthorized)) {
        if let Ok(mut tokens) = tokens.lock() {
            tokens.forget();
        }
        outcome = attempt();
    }
    // A file changed while the sync worked on it: taken as it is now on one more try.
    if let Err(SyncError::Failed(error)) = &outcome
        && rnote_sync::failure_kind(error) == rnote_sync::FailureKind::ChangedMeanwhile
    {
        outcome = attempt();
    }
    match outcome {
        Ok(report) => SyncStatus::Done {
            at: SystemTime::now(),
            report,
        },
        Err(SyncError::Unauthorized) if store.load().ok().flatten() != token_at_start => {
            SyncStatus::Pending
        }
        Err(SyncError::Unauthorized) => {
            if let Err(error) = store.delete() {
                warn!("Forgetting the Google Drive token failed: {error:?}");
            }
            SyncStatus::SignInNeeded
        }
        // Another process syncs this library: the next trigger tries again.
        Err(SyncError::Busy | SyncError::Offline(_)) => SyncStatus::Pending,
        // `{:#}` keeps the whole context chain in a single line.
        Err(SyncError::Failed(error)) => {
            warn!("Syncing failed: {error:?}");
            SyncStatus::Failed(super::messages::failure_message(&error))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use rnote_sync::oauth::{AccessTokens, Tokens};
    use rnote_sync::{SyncError, SyncReport};

    use super::run_with_retry;
    use crate::sync::credentials::{MemoryStore, RefreshTokenStore};
    use crate::sync::status::SyncStatus;

    fn tokens() -> Arc<Mutex<AccessTokens>> {
        Arc::new(Mutex::new(AccessTokens::new(|| {
            Ok(Tokens {
                access_token: "a".to_owned(),
                expires_in: Duration::from_secs(3600),
                refresh_token: None,
            })
        })))
    }

    #[test]
    fn unauthorized_twice_needs_sign_in() {
        let store = MemoryStore::default();
        store.save("r").unwrap();
        let mut attempts = 0;

        let status = run_with_retry(&tokens(), &store, || {
            attempts += 1;
            Err(SyncError::Unauthorized)
        });

        assert_eq!(status, SyncStatus::SignInNeeded);
        assert_eq!(attempts, 2);
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn signing_in_again_during_the_sync_keeps_the_new_token() {
        let store = MemoryStore::default();
        store.save("old").unwrap();

        // Signed out and in again while the sync ran with the old token.
        let status = run_with_retry(&tokens(), &store, || {
            store.save("new").unwrap();
            Err(SyncError::Unauthorized)
        });

        assert_eq!(store.load().unwrap().as_deref(), Some("new"));
        assert_eq!(status, SyncStatus::Pending);
    }

    #[test]
    fn unauthorized_once_retries_with_a_new_token() {
        let store = MemoryStore::default();
        store.save("r").unwrap();
        let mut attempts = 0;

        let status = run_with_retry(&tokens(), &store, || {
            attempts += 1;
            if attempts == 1 {
                Err(SyncError::Unauthorized)
            } else {
                Ok(SyncReport::default())
            }
        });

        assert!(matches!(status, SyncStatus::Done { .. }), "{status:?}");
        assert_eq!(store.load().unwrap().as_deref(), Some("r"));
    }

    #[test]
    fn offline_is_pending_and_failures_say_why() {
        let store = MemoryStore::default();

        let offline = run_with_retry(&tokens(), &store, || {
            Err(SyncError::Offline("io: x".into()))
        });
        let failed = run_with_retry(&tokens(), &store, || {
            Err(SyncError::Failed(anyhow::anyhow!("disk full")))
        });

        assert_eq!(offline, SyncStatus::Pending);
        assert_eq!(
            failed,
            SyncStatus::Failed("Something unexpected went wrong (disk full)".to_owned())
        );
    }

    #[test]
    fn a_file_that_changed_meanwhile_is_synced_on_one_more_try() {
        let store = MemoryStore::default();
        let mut attempts = 0;

        let status = run_with_retry(&tokens(), &store, || {
            attempts += 1;
            if attempts == 1 {
                Err(SyncError::Failed(
                    rnote_sync::ChangedDuringSync("a.rnote changed here during the sync".into())
                        .into(),
                ))
            } else {
                Ok(rnote_sync::SyncReport::default())
            }
        });

        assert_eq!(attempts, 2);
        assert!(matches!(status, SyncStatus::Done { .. }));
    }

    #[test]
    fn a_full_drive_is_said_plainly() {
        let store = MemoryStore::default();
        let body = br#"{"error": {"errors": [{"reason": "storageQuotaExceeded"}]}}"#;

        let status = run_with_retry(&tokens(), &store, || {
            Err(SyncError::Failed(
                rnote_sync::DriveRefusal::new(403, body).into(),
            ))
        });

        assert_eq!(
            status,
            SyncStatus::Failed("Your Google Drive is full".to_owned())
        );
    }
}
