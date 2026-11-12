//! The OAuth client this build was made with, and where the refresh token is kept.

#[cfg(test)]
use std::sync::Mutex;

use rnote_sync::oauth::OAuthClient;

/// The user name the refresh token is kept under, in the keyring entry of the app.
const KEYRING_USER: &str = "google-drive";

/// The OAuth client this build was compiled with, from `RNOTE_GDRIVE_CLIENT_ID` and
/// `RNOTE_GDRIVE_CLIENT_SECRET`; none without them.
pub(crate) fn build_client() -> Option<OAuthClient> {
    client_from(
        option_env!("RNOTE_GDRIVE_CLIENT_ID"),
        option_env!("RNOTE_GDRIVE_CLIENT_SECRET"),
    )
}

/// The client of `id` and `secret`, if both are given and not empty.
pub(crate) fn client_from(id: Option<&str>, secret: Option<&str>) -> Option<OAuthClient> {
    match (id, secret) {
        (Some(id), Some(secret)) if !id.is_empty() && !secret.is_empty() => Some(OAuthClient {
            id: id.to_owned(),
            secret: secret.to_owned(),
        }),
        _ => None,
    }
}

/// Where the refresh token lives between runs.
pub(crate) trait RefreshTokenStore: Send + Sync {
    /// The token, if signed in.
    fn load(&self) -> anyhow::Result<Option<String>>;
    /// Keeps `token`, replacing any other.
    fn save(&self, token: &str) -> anyhow::Result<()>;
    /// Forgets the token; fine when there's none.
    fn delete(&self) -> anyhow::Result<()>;
}

/// The system keyring: Secret Service on Linux (also inside Flatpak), Credential Manager
/// on Windows, Keychain on macOS.
#[derive(Debug)]
pub(crate) struct KeyringStore {
    service: String,
}

impl KeyringStore {
    /// The entry of the app `service` (its app id).
    pub(crate) fn new(service: &str) -> Self {
        Self {
            service: service.to_owned(),
        }
    }

    fn entry(&self) -> anyhow::Result<keyring::Entry> {
        Ok(keyring::Entry::new(&self.service, KEYRING_USER)?)
    }
}

impl RefreshTokenStore for KeyringStore {
    fn load(&self) -> anyhow::Result<Option<String>> {
        match self.entry()?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn save(&self, token: &str) -> anyhow::Result<()> {
        Ok(self.entry()?.set_password(token)?)
    }

    fn delete(&self) -> anyhow::Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

/// A store in memory, for tests.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct MemoryStore(Mutex<Option<String>>);

#[cfg(test)]
impl RefreshTokenStore for MemoryStore {
    fn load(&self) -> anyhow::Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| anyhow::anyhow!("poisoned"))?
            .clone())
    }

    fn save(&self, token: &str) -> anyhow::Result<()> {
        *self.0.lock().map_err(|_| anyhow::anyhow!("poisoned"))? = Some(token.to_owned());
        Ok(())
    }

    fn delete(&self) -> anyhow::Result<()> {
        *self.0.lock().map_err(|_| anyhow::anyhow!("poisoned"))? = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryStore, RefreshTokenStore, client_from};

    #[test]
    fn empty_credentials_mean_no_sync() {
        assert!(client_from(None, Some("s")).is_none());
        assert!(client_from(Some("i"), None).is_none());
        assert!(client_from(Some(""), Some("s")).is_none());
        let client = client_from(Some("i"), Some("s")).unwrap();
        assert_eq!((client.id.as_str(), client.secret.as_str()), ("i", "s"));
    }

    #[test]
    fn memory_store_keeps_one_token() {
        let store = MemoryStore::default();

        assert_eq!(store.load().unwrap(), None);
        store.save("r").unwrap();
        assert_eq!(store.load().unwrap().as_deref(), Some("r"));
        store.delete().unwrap();
        store.delete().unwrap();
        assert_eq!(store.load().unwrap(), None);
    }
}
