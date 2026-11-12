//! Signing in from an installed app: PKCE, the redirect to this computer and the tokens.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::stub_http::Stub;
use crate::oauth::{
    AccessTokens, OAuthClient, OAuthError, Pkce, SCOPE, Tokens, authorization_url,
    exchange_code_at, refresh_at, wait_for_redirect,
};

const JSON: &str = "application/json";

fn client() -> OAuthClient {
    OAuthClient { id: "id-1".to_owned(), secret: "secret-1".to_owned() }
}

/// Sends `request_line` to `port` like a browser would, and returns what came back.
fn browse(port: u16, request_line: &str) -> JoinHandle<String> {
    let request = format!("{request_line}\r\nHost: 127.0.0.1\r\nAccept: text/html\r\n\r\n");
    std::thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        answer
    })
}

fn listener() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

#[test]
fn pkce_matches_rfc_7636() {
    let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");

    assert_eq!(pkce.challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    assert_eq!(Pkce::new().verifier.len(), 43);
    assert_ne!(Pkce::new().verifier, Pkce::new().verifier);
}

#[test]
fn authorization_url_asks_only_for_drive_file() {
    let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");

    let url = authorization_url(&client(), "http://127.0.0.1:4321", &pkce, "state-1");

    assert_eq!(
        url.as_str().split('?').next(),
        Some("https://accounts.google.com/o/oauth2/v2/auth")
    );
    let query: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    let value = |key: &str| query.iter().find(|(name, _)| name == key).map(|(_, v)| v.as_str());
    assert_eq!(value("client_id"), Some("id-1"));
    assert_eq!(value("redirect_uri"), Some("http://127.0.0.1:4321"));
    assert_eq!(value("response_type"), Some("code"));
    assert_eq!(value("scope"), Some(SCOPE));
    assert_eq!(SCOPE, "https://www.googleapis.com/auth/drive.file");
    assert_eq!(value("code_challenge"), Some("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"));
    assert_eq!(value("code_challenge_method"), Some("S256"));
    assert_eq!(value("state"), Some("state-1"));
    assert_eq!(value("access_type"), Some("offline"));
    assert_eq!(value("prompt"), Some("consent"));
    assert_eq!(query.len(), 9);
}

#[test]
fn redirect_returns_the_code() {
    let (listener, port) = listener();
    let browser = browse(port, "GET /?state=s&code=c%2F1 HTTP/1.1");

    let code = wait_for_redirect(&listener, "s", Duration::from_secs(5));

    assert_eq!(code.unwrap(), "c/1");
    assert!(browser.join().unwrap().starts_with("HTTP/1.1 200"));
}

#[test]
fn redirect_ignores_other_paths() {
    let (listener, port) = listener();
    let waiting =
        std::thread::spawn(move || wait_for_redirect(&listener, "s", Duration::from_secs(5)));

    let favicon = browse(port, "GET /favicon.ico HTTP/1.1").join().unwrap();
    browse(port, "GET /?state=s&code=c HTTP/1.1").join().unwrap();

    assert!(favicon.starts_with("HTTP/1.1 404"), "{favicon}");
    assert_eq!(waiting.join().unwrap().unwrap(), "c");
}

#[test]
fn redirect_rejects_another_state() {
    let (listener, port) = listener();
    let browser = browse(port, "GET /?state=x&code=c HTTP/1.1");

    let code = wait_for_redirect(&listener, "s", Duration::from_secs(5));

    assert!(matches!(code, Err(OAuthError::StateMismatch)), "{code:?}");
    browser.join().unwrap();
}

#[test]
fn redirect_reports_a_denial() {
    let (listener, port) = listener();
    let browser = browse(port, "GET /?state=s&error=access_denied HTTP/1.1");

    let code = wait_for_redirect(&listener, "s", Duration::from_secs(5));

    assert!(matches!(code, Err(OAuthError::Denied)), "{code:?}");
    browser.join().unwrap();
}

#[test]
fn redirect_times_out() {
    let (listener, _port) = listener();
    let started = Instant::now();

    let code = wait_for_redirect(&listener, "s", Duration::from_millis(200));

    assert!(matches!(code, Err(OAuthError::TimedOut)), "{code:?}");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn code_exchange_sends_the_verifier() {
    let stub = Stub::start(vec![(
        200,
        JSON,
        br#"{"access_token":"a","expires_in":3599,"refresh_token":"r","token_type":"Bearer"}"#
            .to_vec(),
    )]);
    let pkce = Pkce::from_verifier("verifier-1");

    let tokens =
        exchange_code_at(stub.url(), &client(), "code-1", &pkce, "http://127.0.0.1:4321").unwrap();

    assert_eq!(tokens.access_token, "a");
    assert_eq!(tokens.expires_in, Duration::from_secs(3599));
    assert_eq!(tokens.refresh_token.as_deref(), Some("r"));
    let request = &stub.requests()[0];
    assert_eq!((request.method.as_str(), request.path()), ("POST", "/token"));
    let form: Vec<(String, String)> =
        url::form_urlencoded::parse(&request.body).into_owned().collect();
    let value = |key: &str| form.iter().find(|(name, _)| name == key).map(|(_, v)| v.as_str());
    assert_eq!(value("code"), Some("code-1"));
    assert_eq!(value("code_verifier"), Some("verifier-1"));
    assert_eq!(value("grant_type"), Some("authorization_code"));
    assert_eq!(value("client_id"), Some("id-1"));
    assert_eq!(value("client_secret"), Some("secret-1"));
    assert_eq!(value("redirect_uri"), Some("http://127.0.0.1:4321"));
}

#[test]
fn refresh_sends_the_refresh_token() {
    let stub =
        Stub::start(vec![(200, JSON, br#"{"access_token":"b","expires_in":3599}"#.to_vec())]);

    let tokens = refresh_at(stub.url(), &client(), "r").unwrap();

    assert_eq!(tokens.access_token, "b");
    assert_eq!(tokens.refresh_token, None);
    let form: Vec<(String, String)> =
        url::form_urlencoded::parse(&stub.requests()[0].body).into_owned().collect();
    assert!(form.contains(&("grant_type".to_owned(), "refresh_token".to_owned())));
    assert!(form.contains(&("refresh_token".to_owned(), "r".to_owned())));
}

#[test]
fn refresh_reports_a_revoked_grant() {
    let stub = Stub::start(vec![(400, JSON, br#"{"error":"invalid_grant"}"#.to_vec())]);

    let tokens = refresh_at(stub.url(), &client(), "r");

    assert!(matches!(tokens, Err(OAuthError::Revoked)), "{tokens:?}");
}

#[test]
fn refresh_server_error_fails() {
    let stub = Stub::start(vec![(500, JSON, b"{}".to_vec())]);

    assert!(matches!(refresh_at(stub.url(), &client(), "r"), Err(OAuthError::Failed(_))));
}

#[test]
fn refresh_offline() {
    let tokens = refresh_at("http://127.0.0.1:9", &client(), "r");

    assert!(matches!(tokens, Err(OAuthError::Offline(_))), "{tokens:?}");
}

/// Access tokens that each last an hour, counting how many were asked for.
fn hourly_tokens() -> (AccessTokens, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = std::sync::Arc::clone(&asked);
    let tokens = AccessTokens::new(move || {
        let n = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        Ok(Tokens {
            access_token: format!("token-{n}"),
            expires_in: Duration::from_secs(3600),
            refresh_token: None,
        })
    });
    (tokens, asked)
}

#[test]
fn access_tokens_are_reused_until_a_minute_before_expiry() {
    let (mut tokens, asked) = hourly_tokens();
    let start = Instant::now();

    assert_eq!(tokens.get(start).unwrap(), "token-1");
    assert_eq!(tokens.get(start + Duration::from_secs(3500)).unwrap(), "token-1");
    assert_eq!(tokens.get(start + Duration::from_secs(3541)).unwrap(), "token-2");
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn forgotten_access_token_is_renewed() {
    let (mut tokens, _asked) = hourly_tokens();
    let start = Instant::now();

    tokens.get(start).unwrap();
    tokens.forget();

    assert_eq!(tokens.get(start).unwrap(), "token-2");
}
