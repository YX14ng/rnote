//! Signing out: Google forgets the app's access to the account.

use anyhow::anyhow;
use ureq::Agent;

use crate::drive::transport_error;
use crate::remote::RemoteError;

/// Google's OAuth endpoint, unless a test points elsewhere.
const DEFAULT_BASE_URL: &str = "https://oauth2.googleapis.com";

/// Revokes the access `token`, and with it the app's access to the account.
pub fn revoke(token: &str) -> Result<(), RemoteError> {
    revoke_at(DEFAULT_BASE_URL, token)
}

/// Like [`revoke`], against `base_url` instead of Google (for tests).
pub fn revoke_at(base_url: &str, token: &str) -> Result<(), RemoteError> {
    let agent: Agent = Agent::config_builder().http_status_as_error(false).build().into();
    let body: String =
        url::form_urlencoded::Serializer::new(String::new()).append_pair("token", token).finish();
    let url = format!("{}/revoke", base_url.trim_end_matches('/'));
    let response = agent
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body)
        .map_err(transport_error)?;
    match response.status().as_u16() {
        200..=299 => Ok(()),
        // An expired or already revoked token: there's nothing left to revoke.
        400 => Ok(()),
        status => Err(RemoteError::Failed(anyhow!("Google refused to sign out ({status})"))),
    }
}
