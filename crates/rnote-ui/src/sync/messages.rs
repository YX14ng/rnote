//! What the sync says when something goes wrong, in the user's language.

use gettextrs::{gettext, ngettext};
use rnote_sync::FailureKind;
use rnote_sync::oauth::OAuthError;

use crate::library::error_text::user_message;

/// Why a sync failed.
pub(crate) fn failure_message(error: &anyhow::Error) -> String {
    match rnote_sync::failure_kind(error) {
        FailureKind::DriveFull => gettext("Your Google Drive is full"),
        FailureKind::DriveRefused => gettext("Google Drive refused a change"),
        FailureKind::ChangedMeanwhile => {
            gettext("A notebook changed while it was syncing; the next sync takes it")
        }
        FailureKind::LocalFiles | FailureKind::Unexpected => user_message(error),
    }
}

/// Why signing in failed.
pub(crate) fn sign_in_message(error: &OAuthError) -> String {
    match error {
        OAuthError::Denied => gettext("Access to Google Drive wasn't allowed"),
        OAuthError::StateMismatch => gettext("Google answered another sign-in: try again"),
        OAuthError::TimedOut => gettext("Signing in took too long"),
        OAuthError::Cancelled => gettext("The browser couldn't be opened"),
        OAuthError::Revoked => gettext("The access to Google Drive was removed"),
        OAuthError::Offline(_) => gettext("Google can't be reached"),
        OAuthError::Failed(error) => user_message(error),
    }
}

/// What to say about the notebooks a sync couldn't take this time (`skipped` entries read
/// `path: reason`); `None` if there are none.
pub(crate) fn skipped_message(skipped: &[String]) -> Option<String> {
    if skipped.is_empty() {
        return None;
    }
    let names: Vec<&str> = skipped
        .iter()
        .map(|entry| {
            entry
                .split_once(": ")
                .map_or(entry.as_str(), |(path, _)| path)
        })
        .collect();
    let count = u32::try_from(names.len()).unwrap_or(u32::MAX);
    Some(
        ngettext(
            "{n} notebook couldn't be synced this time: {names}. It tries again on the next sync.",
            "{n} notebooks couldn't be synced this time: {names}. They try again on the next sync.",
            count,
        )
        .replace("{n}", &names.len().to_string())
        .replace("{names}", &names.join(", ")),
    )
}

#[cfg(test)]
mod tests {
    use rnote_sync::oauth::OAuthError;

    use super::{sign_in_message, skipped_message};

    #[test]
    fn skipped_notebooks_are_named() {
        assert_eq!(skipped_message(&[]), None);
        assert_eq!(
            skipped_message(&["Orga/a.rnote: Writing failed".to_owned()]).as_deref(),
            Some(
                "1 notebook couldn't be synced this time: Orga/a.rnote. It tries again on the next sync."
            )
        );
    }

    #[test]
    fn a_sign_in_that_was_not_allowed_says_so() {
        assert_eq!(
            sign_in_message(&OAuthError::Denied),
            "Access to Google Drive wasn't allowed"
        );
    }
}
