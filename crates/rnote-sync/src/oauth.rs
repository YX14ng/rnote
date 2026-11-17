//! Signing in to Google from an installed app (the desktop): OAuth 2.0 with PKCE and a
//! redirect to a port on this computer, then keeping an access token fresh with the
//! refresh token. Android gets its tokens from Play Services instead.

use std::fmt;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use ureq::Agent;
use url::Url;

use crate::drive::transport_error;
use crate::remote::RemoteError;

/// The only access asked for: the files the app creates, nothing else of the Drive.
pub const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

/// Where Google asks the user, in the browser.
const AUTHORIZATION_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
/// Google's token endpoint, unless a test points elsewhere.
const DEFAULT_BASE_URL: &str = "https://oauth2.googleapis.com";
/// How long a token request may take before Google counts as unreachable.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(30);
/// How long before it expires an access token is renewed.
const RENEW_MARGIN: Duration = Duration::from_secs(60);
/// How often [`wait_for_redirect`] looks for the browser.
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// How long the browser gets to send its request once connected.
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// What the browser shows once it handed over the answer.
const DONE_PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>Rnote</title>\
<p>Listo: ya podés volver a Rnote.</p><p>Done: you can go back to Rnote.</p>";
/// The page the browser shows when the sign-in didn't go through.
const NOT_DONE_PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>Rnote</title>\
<p>No se inició sesión: volvé a Rnote para intentarlo de nuevo.</p>\
<p>Not signed in: go back to Rnote to try again.</p>";

/// A "Desktop app" OAuth client of the Google Cloud project.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthClient {
    /// The client id.
    pub id: String,
    /// The client secret, which Google doesn't treat as confidential for installed apps.
    pub secret: String,
}

/// A PKCE pair: the verifier stays here, the challenge goes to Google with the request.
#[derive(Clone)]
pub struct Pkce {
    /// The secret proving this app asked for the code.
    pub verifier: String,
    /// The SHA-256 of the verifier, in base64url without padding.
    pub challenge: String,
}

impl Pkce {
    /// A new pair with a verifier of 32 random bytes.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::from_verifier(&random_base64::<32>())
    }

    /// The pair of a known `verifier`.
    pub fn from_verifier(verifier: &str) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self { verifier: verifier.to_owned(), challenge }
    }
}

/// A random value for the `state` parameter, which ties the redirect to this request.
pub fn random_state() -> String {
    random_base64::<16>()
}

fn random_base64<const N: usize>() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; N]>())
}

/// The page where the user lets the app use their Drive, redirecting to `redirect_uri`.
pub fn authorization_url(
    client: &OAuthClient,
    redirect_uri: &str,
    pkce: &Pkce,
    state: &str,
) -> Url {
    let mut url = Url::parse(AUTHORIZATION_URL).expect("The authorization URL is valid");
    url.query_pairs_mut()
        .append_pair("client_id", &client.id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        // A refresh token, so the user signs in once.
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    url
}

/// Why signing in or renewing the access stopped.
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    /// The user didn't let the app use their Drive.
    #[error("Access to Google Drive wasn't granted")]
    Denied,
    /// The redirect belonged to another sign-in.
    #[error("The answer from Google belonged to another sign-in")]
    StateMismatch,
    /// The browser never came back.
    #[error("Signing in took too long")]
    TimedOut,
    /// The app stopped waiting, e.g. because the browser couldn't be opened.
    #[error("Signing in was cancelled")]
    Cancelled,
    /// The access was removed, e.g. from the Google account: sign in again.
    #[error("The access to Google Drive was removed")]
    Revoked,
    /// Google can't be reached.
    #[error("Google can't be reached: {0}")]
    Offline(String),
    /// Anything else.
    #[error(transparent)]
    Failed(#[from] anyhow::Error),
}

impl From<RemoteError> for OAuthError {
    fn from(error: RemoteError) -> Self {
        match error {
            RemoteError::Offline(message) => Self::Offline(message),
            RemoteError::Unauthorized => Self::Revoked,
            RemoteError::Failed(error) => Self::Failed(error),
        }
    }
}

/// Waits up to `timeout` for the browser to come back to `listener` from Google, and
/// returns the code it brings. Requests for anything but `/` (e.g. `/favicon.ico`) get a
/// 404 and the wait goes on. Setting `cancelled` stops the wait.
pub fn wait_for_redirect(
    listener: &TcpListener,
    state: &str,
    timeout: Duration,
    cancelled: &AtomicBool,
) -> Result<String, OAuthError> {
    listener.set_nonblocking(true).map_err(|error| anyhow!(error))?;
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(answer) = answer_browser(stream, state)? {
                    return answer;
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                if cancelled.load(Ordering::SeqCst) {
                    return Err(OAuthError::Cancelled);
                }
                if Instant::now() >= deadline {
                    return Err(OAuthError::TimedOut);
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(error) => return Err(OAuthError::Failed(anyhow!(error))),
        }
    }
}

/// Reads one request of the browser and answers it; `None` when it wasn't the redirect.
fn answer_browser(
    mut stream: TcpStream,
    state: &str,
) -> Result<Option<Result<String, OAuthError>>, OAuthError> {
    let io = |error: std::io::Error| OAuthError::Failed(anyhow!(error));
    stream.set_nonblocking(false).map_err(io)?;
    stream.set_read_timeout(Some(READ_TIMEOUT)).map_err(io)?;
    let mut reader = BufReader::new(stream.try_clone().map_err(io)?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return Ok(None);
    }
    // The headers say nothing needed; they are read so the browser gets its answer.
    let mut header = String::new();
    while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
        header.clear();
    }
    let target = request_line.split_whitespace().nth(1).unwrap_or_default();
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        respond(&mut stream, "400 Bad Request", "");
        return Ok(None);
    };
    if url.path() != "/" {
        respond(&mut stream, "404 Not Found", "");
        return Ok(None);
    }
    let value = |key: &str| {
        url.query_pairs().find(|(name, _)| name == key).map(|(_, value)| value.into_owned())
    };
    // Not Google's answer at all (e.g. the browser asking for the page by itself).
    if value("code").is_none() && value("error").is_none() {
        respond(&mut stream, "400 Bad Request", "");
        return Ok(None);
    }
    let answer = if value("state").as_deref() != Some(state) {
        Err(OAuthError::StateMismatch)
    } else if value("error").is_some() {
        Err(OAuthError::Denied)
    } else {
        Ok(value("code").unwrap_or_default())
    };
    let page = if answer.is_ok() { DONE_PAGE } else { NOT_DONE_PAGE };
    respond(&mut stream, "200 OK", page);
    Ok(Some(answer))
}

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let answer = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    // The browser may be gone; the answer to the app doesn't depend on it.
    let _ = stream.write_all(answer.as_bytes());
    let _ = stream.flush();
}

/// What Google hands out for a code or a refresh token.
#[derive(Clone, PartialEq, Eq)]
pub struct Tokens {
    /// The token Drive takes.
    pub access_token: String,
    /// How long the access token lasts.
    pub expires_in: Duration,
    /// The token that gets new access tokens; only when signing in.
    pub refresh_token: Option<String>,
}

// Printed without the secrets, so a log line never carries them.
impl fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthClient").field("id", &self.id).finish_non_exhaustive()
    }
}

impl fmt::Debug for Pkce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pkce").field("challenge", &self.challenge).finish_non_exhaustive()
    }
}

impl fmt::Debug for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tokens")
            .field("expires_in", &self.expires_in)
            .field("has_refresh_token", &self.refresh_token.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct ErrorAnswer {
    error: String,
}

/// Trades the `code` from the redirect for the tokens.
pub fn exchange_code(
    client: &OAuthClient,
    code: &str,
    pkce: &Pkce,
    redirect_uri: &str,
) -> Result<Tokens, OAuthError> {
    exchange_code_at(DEFAULT_BASE_URL, client, code, pkce, redirect_uri)
}

/// Like [`exchange_code`], against `base_url` instead of Google (for tests).
pub fn exchange_code_at(
    base_url: &str,
    client: &OAuthClient,
    code: &str,
    pkce: &Pkce,
    redirect_uri: &str,
) -> Result<Tokens, OAuthError> {
    post_token(
        base_url,
        TOKEN_TIMEOUT,
        &[
            ("code", code),
            ("client_id", &client.id),
            ("client_secret", &client.secret),
            ("code_verifier", &pkce.verifier),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
        ],
    )
}

/// A new access token from the `refresh_token`.
pub fn refresh(client: &OAuthClient, refresh_token: &str) -> Result<Tokens, OAuthError> {
    refresh_at(DEFAULT_BASE_URL, client, refresh_token)
}

/// Like [`refresh`], against `base_url` instead of Google (for tests).
pub fn refresh_at(
    base_url: &str,
    client: &OAuthClient,
    refresh_token: &str,
) -> Result<Tokens, OAuthError> {
    refresh_with_timeout(base_url, client, refresh_token, TOKEN_TIMEOUT)
}

/// Like [`refresh_at`], giving up after `timeout`.
pub(crate) fn refresh_with_timeout(
    base_url: &str,
    client: &OAuthClient,
    refresh_token: &str,
    timeout: Duration,
) -> Result<Tokens, OAuthError> {
    post_token(
        base_url,
        timeout,
        &[
            ("client_id", &client.id),
            ("client_secret", &client.secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ],
    )
}

fn post_token(
    base_url: &str,
    timeout: Duration,
    form: &[(&str, &str)],
) -> Result<Tokens, OAuthError> {
    let agent: Agent = Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(timeout))
        .build()
        .into();
    let body = url::form_urlencoded::Serializer::new(String::new()).extend_pairs(form).finish();
    let url = format!("{}/token", base_url.trim_end_matches('/'));
    let mut response = agent
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body)
        .map_err(|error| OAuthError::from(transport_error(error)))?;
    let status = response.status().as_u16();
    let bytes = response
        .body_mut()
        .read_to_vec()
        .map_err(|error| OAuthError::from(transport_error(error)))?;
    if (200..300).contains(&status) {
        let answer: TokenAnswer = serde_json::from_slice(&bytes)
            .map_err(|error| anyhow!(error).context("Reading Google's tokens"))?;
        return Ok(Tokens {
            access_token: answer.access_token,
            expires_in: Duration::from_secs(answer.expires_in),
            refresh_token: answer.refresh_token,
        });
    }
    let error = serde_json::from_slice::<ErrorAnswer>(&bytes).map(|answer| answer.error);
    match error {
        Ok(error) if error == "invalid_grant" => Err(OAuthError::Revoked),
        Ok(error) => {
            Err(OAuthError::Failed(anyhow!("Google refused the sign-in ({status}): {error}")))
        }
        Err(_) => Err(OAuthError::Failed(anyhow!("Google refused the sign-in ({status})"))),
    }
}

/// The access token for Drive, renewed with `renew` only when there's none or it's about
/// to expire.
pub struct AccessTokens {
    renew: Box<dyn FnMut() -> Result<Tokens, OAuthError> + Send>,
    current: Option<(String, Instant)>,
}

impl AccessTokens {
    /// Tokens that `renew` gets, e.g. with [`refresh`].
    pub fn new(renew: impl FnMut() -> Result<Tokens, OAuthError> + Send + 'static) -> Self {
        Self { renew: Box::new(renew), current: None }
    }

    /// An access token valid for at least a minute after `now`.
    pub fn get(&mut self, now: Instant) -> Result<String, OAuthError> {
        if let Some((token, expires_at)) = &self.current
            && now + RENEW_MARGIN < *expires_at
        {
            return Ok(token.clone());
        }
        let tokens = (self.renew)()?;
        self.current = Some((tokens.access_token.clone(), now + tokens.expires_in));
        Ok(tokens.access_token)
    }

    /// Drops the current access token, e.g. after Drive refused it.
    pub fn forget(&mut self) {
        self.current = None;
    }
}

impl std::fmt::Debug for AccessTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the token itself.
        f.debug_struct("AccessTokens").field("has_token", &self.current.is_some()).finish()
    }
}
