//! Where the sync stands, and how the sync button words it.

use std::time::{Duration, SystemTime};

use gettextrs::{gettext, ngettext};
use rnote_sync::SyncReport;

/// Where the sync stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncStatus {
    /// This build has no OAuth client, so it can't sign in.
    Unavailable,
    /// Not signed in.
    SignedOut,
    /// Signed in before, but Google needs the user again (e.g. access removed).
    SignInNeeded,
    /// Signed in, before the first sync of this run.
    Waiting,
    /// A sync is running.
    Running,
    /// The last sync ended well at `at`.
    Done {
        /// When it ended.
        at: SystemTime,
        /// What it did.
        report: SyncReport,
    },
    /// Google couldn't be reached; the next trigger tries again.
    Pending,
    /// The last sync failed, with why.
    Failed(String),
}

impl SyncStatus {
    /// Whether syncs can run: signed in, with a client.
    pub(crate) fn signed_in(&self) -> bool {
        !matches!(
            self,
            Self::Unavailable | Self::SignedOut | Self::SignInNeeded
        )
    }
}

/// How long ago something was, in the steps the menu words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ago {
    JustNow,
    Minutes(u64),
    Hours(u64),
    /// A day or more: the date says it better.
    OnDate,
}

impl Ago {
    pub(crate) fn of(elapsed: Duration) -> Self {
        let seconds = elapsed.as_secs();
        match seconds {
            0..60 => Self::JustNow,
            60..3600 => Self::Minutes(seconds / 60),
            3600..86_400 => Self::Hours(seconds / 3600),
            _ => Self::OnDate,
        }
    }
}

/// What the sync menu says about `status` at `now`.
pub(crate) fn status_text(status: &SyncStatus, now: SystemTime) -> String {
    match status {
        SyncStatus::Unavailable => {
            gettext("This build of Rnote has no Google Drive credentials, so it can't sync")
        }
        SyncStatus::SignedOut => gettext("Not signed in to Google Drive"),
        SyncStatus::SignInNeeded => gettext("Sign in to Google Drive again to keep syncing"),
        SyncStatus::Waiting => gettext("Waiting to sync"),
        SyncStatus::Running => gettext("Syncing…"),
        SyncStatus::Done { at, .. } => {
            // A clock that went back counts as just now.
            let elapsed = now.duration_since(*at).unwrap_or_default();
            gettext("Synced {when}").replace("{when}", &ago_text(Ago::of(elapsed), *at))
        }
        SyncStatus::Pending => gettext("Offline: it syncs once Google can be reached"),
        SyncStatus::Failed(message) => gettext("Sync failed: {error}").replace("{error}", message),
    }
}

fn ago_text(ago: Ago, at: SystemTime) -> String {
    let count = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    match ago {
        Ago::JustNow => gettext("just now"),
        Ago::Minutes(n) => {
            ngettext("{n} minute ago", "{n} minutes ago", count(n)).replace("{n}", &n.to_string())
        }
        Ago::Hours(n) => {
            ngettext("{n} hour ago", "{n} hours ago", count(n)).replace("{n}", &n.to_string())
        }
        Ago::OnDate => {
            let seconds = at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let date = gtk4::glib::DateTime::from_unix_local(seconds as i64)
                .and_then(|date| date.format("%x %H:%M"))
                .map(|date| date.to_string())
                .unwrap_or_default();
            gettext("on {date}").replace("{date}", &date)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{Ago, SyncStatus, status_text};

    #[test]
    fn ago_steps() {
        let ago = |seconds| Ago::of(Duration::from_secs(seconds));

        assert_eq!(ago(0), Ago::JustNow);
        assert_eq!(ago(59), Ago::JustNow);
        assert_eq!(ago(60), Ago::Minutes(1));
        assert_eq!(ago(3599), Ago::Minutes(59));
        assert_eq!(ago(3600), Ago::Hours(1));
        assert_eq!(ago(86_399), Ago::Hours(23));
        assert_eq!(ago(86_400), Ago::OnDate);
    }

    #[test]
    fn pending_says_offline_not_the_error() {
        let text = status_text(&SyncStatus::Pending, SystemTime::now());

        assert!(text.contains("Offline"), "{text}");
        assert!(!text.contains("io:"), "{text}");
    }

    #[test]
    fn done_says_just_now_not_zero_minutes() {
        let now = SystemTime::now();
        let done = SyncStatus::Done {
            at: now,
            report: Default::default(),
        };

        assert_eq!(
            status_text(&done, now + Duration::from_secs(5)),
            "Synced just now"
        );
        assert_eq!(
            status_text(&done, now + Duration::from_secs(120)),
            "Synced 2 minutes ago"
        );
    }
}
