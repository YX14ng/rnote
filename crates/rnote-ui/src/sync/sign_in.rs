//! One sign-in at a time: clicking "Sign In" again while the browser is on Google's page
//! opens that same page again instead of starting a second sign-in.

/// The sign-in in progress, if any.
#[derive(Debug, Default)]
pub(crate) struct PendingSignIn {
    url: Option<String>,
}

impl PendingSignIn {
    /// Starts a sign-in at `url`, unless one is in progress: then returns that one's page,
    /// to show again.
    pub(crate) fn start(&mut self, url: &str) -> Option<String> {
        match &self.url {
            Some(current) => Some(current.clone()),
            None => {
                self.url = Some(url.to_owned());
                None
            }
        }
    }

    /// The sign-in in progress ended, however it went.
    pub(crate) fn finish(&mut self) {
        self.url = None;
    }

    /// The page of the sign-in in progress.
    #[cfg(test)]
    pub(crate) fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::PendingSignIn;

    #[test]
    fn a_second_click_shows_the_first_page_again() {
        let mut pending = PendingSignIn::default();

        assert_eq!(pending.start("https://first"), None);
        assert_eq!(
            pending.start("https://second").as_deref(),
            Some("https://first")
        );
        assert_eq!(pending.url(), Some("https://first"));
    }

    #[test]
    fn after_it_ends_a_new_sign_in_starts() {
        let mut pending = PendingSignIn::default();
        pending.start("https://first");

        pending.finish();

        assert_eq!(pending.url(), None);
        assert_eq!(pending.start("https://second"), None);
        assert_eq!(pending.url(), Some("https://second"));
    }
}
