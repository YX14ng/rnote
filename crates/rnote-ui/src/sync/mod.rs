//! Syncing the library with Google Drive from the desktop: signing in through the
//! browser, the refresh token in the keyring, and one sync at a time on a worker thread.

pub(crate) mod button;
pub(crate) mod credentials;
mod messages;
pub(crate) mod opened;
pub(crate) mod run;
pub(crate) mod schedule;
pub(crate) mod sign_in;
pub(crate) mod status;

use gettextrs::gettext;
use std::cell::RefCell;
use std::net::TcpListener;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use gtk4::{UriLauncher, Window, gio, gio::prelude::*, glib};
use rnote_sync::oauth::{self, AccessTokens, OAuthClient, OAuthError};
use rnote_sync::{DriveClient, RemoteError, SyncContext};
use tracing::error;

use credentials::{KeyringStore, RefreshTokenStore};
use schedule::Scheduler;
use sign_in::PendingSignIn;
pub(crate) use status::SyncStatus;

/// How long after a save [`RnSync::sync_soon`] waits.
const SOON_DELAY: Duration = Duration::from_secs(5);
/// How often an open app syncs by itself, so changes from other devices come in while the
/// user stays in a notebook.
const PERIODIC_DELAY: Duration = Duration::from_secs(5 * 60);
/// How long closing the window may keep the app running for a sync to finish.
const CLOSE_GRACE: Duration = Duration::from_secs(60);
/// How often closing looks whether the syncs are over.
const CLOSE_POLL: Duration = Duration::from_millis(200);
/// How long the browser gets to come back from Google.
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Called with every new status.
type Listener = Rc<dyn Fn(&SyncStatus)>;
/// Where the library is, asked at the start of every sync.
type RootProvider = Box<dyn Fn() -> Option<PathBuf>>;

/// The sync of the window's library. Cloning shares it.
#[derive(Clone)]
pub(crate) struct RnSync {
    inner: Rc<Inner>,
}

struct Inner {
    client: Option<OAuthClient>,
    store: Arc<dyn RefreshTokenStore>,
    tokens: Arc<Mutex<AccessTokens>>,
    status: RefCell<SyncStatus>,
    scheduler: RefCell<Scheduler>,
    soon: RefCell<Option<glib::SourceId>>,
    periodic: std::cell::Cell<bool>,
    listeners: RefCell<Vec<Listener>>,
    root: RefCell<Option<RootProvider>>,
    signing_in: RefCell<PendingSignIn>,
}

impl std::fmt::Debug for RnSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RnSync")
            .field("status", &*self.inner.status.borrow())
            .finish()
    }
}

impl RnSync {
    /// The sync of the app `app_id`, whose keyring entry keeps the refresh token.
    pub(crate) fn new(app_id: &str) -> Self {
        let client = credentials::build_client();
        let store: Arc<dyn RefreshTokenStore> = Arc::new(KeyringStore::new(app_id));
        // The keyring is read off the main thread: it may wait for the user to unlock it.
        let status = if client.is_some() {
            SyncStatus::Checking
        } else {
            SyncStatus::Unavailable
        };
        let tokens = Arc::new(Mutex::new(access_tokens(
            client.clone(),
            Arc::clone(&store),
        )));
        Self {
            inner: Rc::new(Inner {
                client,
                store,
                tokens,
                status: RefCell::new(status),
                scheduler: RefCell::new(Scheduler::default()),
                soon: RefCell::new(None),
                periodic: std::cell::Cell::new(false),
                listeners: RefCell::new(Vec::new()),
                root: RefCell::new(None),
                signing_in: RefCell::new(PendingSignIn::default()),
            }),
        }
    }

    /// Reads the keyring and, when signed in, syncs: what opening the app does. From then
    /// on it also syncs every few minutes.
    pub(crate) fn start_up(&self) {
        if self.status() != SyncStatus::Checking {
            return;
        }
        self.sync_periodically();
        let store = Arc::clone(&self.inner.store);
        let this = self.clone();
        glib::spawn_future_local(async move {
            let token = gio::spawn_blocking(move || store.load())
                .await
                .unwrap_or_else(|_| Err(anyhow!("Reading the keyring stopped unexpectedly")));
            // Signing in meanwhile wins.
            if this.status() == SyncStatus::Checking {
                this.set_status(status::status_after_load(token));
                this.sync_now();
            }
        });
    }

    /// Reads the keyring again, e.g. after the user unlocked it.
    pub(crate) fn retry_keyring(&self) {
        if self.status() == SyncStatus::KeyringLocked {
            self.set_status(SyncStatus::Checking);
            self.start_up();
        }
    }

    /// Where the library to sync is, asked at the start of every sync.
    pub(crate) fn set_root(&self, root: impl Fn() -> Option<PathBuf> + 'static) {
        *self.inner.root.borrow_mut() = Some(Box::new(root));
    }

    pub(crate) fn status(&self) -> SyncStatus {
        self.inner.status.borrow().clone()
    }

    /// Calls `listener` with every new status.
    pub(crate) fn connect_status(&self, listener: impl Fn(&SyncStatus) + 'static) {
        self.inner.listeners.borrow_mut().push(Rc::new(listener));
    }

    fn set_status(&self, status: SyncStatus) {
        *self.inner.status.borrow_mut() = status.clone();
        // Cloned, so a listener may connect another or change the status.
        let listeners = self.inner.listeners.borrow().clone();
        for listener in listeners {
            listener(&status);
        }
    }

    /// Syncs now, or right after the sync that is running; nothing when signed out.
    pub(crate) fn sync_now(&self) {
        if self.status().signed_in() && self.inner.scheduler.borrow_mut().request() {
            self.start();
        }
    }

    /// Syncs every [`PERIODIC_DELAY`] while signed in, for as long as this sync exists.
    fn sync_periodically(&self) {
        if self.inner.periodic.get() {
            return;
        }
        self.inner.periodic.set(true);
        let weak = Rc::downgrade(&self.inner);
        glib::timeout_add_local(PERIODIC_DELAY, {
            let weak = weak.clone();
            move || {
                let Some(inner) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                Self { inner }.sync_now();
                glib::ControlFlow::Continue
            }
        });
        // Back online: what waited for Google goes now, not at the next periodic sync.
        gio::NetworkMonitor::default().connect_network_changed(move |_, available| {
            if let Some(inner) = weak.upgrade() {
                let sync = Self { inner };
                if retries_on_network(&sync.status(), available) {
                    sync.sync_now();
                }
            }
        });
    }

    /// The window is closing: a sync waiting for its few seconds runs now, and the app keeps
    /// running until it (or one already running) ends, up to [`CLOSE_GRACE`]. Without that, a
    /// notebook saved just before closing reached the other devices only on the next start.
    pub(crate) fn finish_before_quitting(&self, app: &impl IsA<gio::Application>) {
        if let Some(source) = self.inner.soon.borrow_mut().take() {
            source.remove();
            self.sync_now();
        }
        if self.inner.scheduler.borrow().idle() {
            return;
        }
        // Until no sync runs or waits: when one ends, the one a save asked for meanwhile
        // starts right after, and must not be cut off.
        let hold = app.hold();
        let weak = Rc::downgrade(&self.inner);
        let started = std::time::Instant::now();
        glib::timeout_add_local(CLOSE_POLL, move || {
            let busy = weak
                .upgrade()
                .is_some_and(|inner| !inner.scheduler.borrow().idle());
            if busy && started.elapsed() < CLOSE_GRACE {
                return glib::ControlFlow::Continue;
            }
            let _ = &hold;
            glib::ControlFlow::Break
        });
    }

    /// Syncs a few seconds from now, e.g. after a save; another call starts the wait over.
    pub(crate) fn sync_soon(&self) {
        if !self.status().signed_in() {
            return;
        }
        if let Some(source) = self.inner.soon.borrow_mut().take() {
            source.remove();
        }
        let weak = Rc::downgrade(&self.inner);
        let source = glib::timeout_add_local_once(SOON_DELAY, move || {
            if let Some(inner) = weak.upgrade() {
                let sync = Self { inner };
                sync.inner.soon.borrow_mut().take();
                sync.sync_now();
            }
        });
        *self.inner.soon.borrow_mut() = Some(source);
    }

    fn start(&self) {
        let root = self.inner.root.borrow().as_ref().and_then(|root| root());
        let Some(root) = root else {
            self.finish(None);
            return;
        };
        self.set_status(SyncStatus::Running);
        let tokens = Arc::clone(&self.inner.tokens);
        let store = Arc::clone(&self.inner.store);
        let state_path = schedule::state_path(&glib::user_data_dir(), &root);
        let device = glib::host_name().to_string();
        let this = self.clone();
        glib::spawn_future_local(async move {
            let status = gio::spawn_blocking(move || {
                run::run_with_retry(&tokens, store.as_ref(), || {
                    let tokens = Arc::clone(&tokens);
                    let mut drive = DriveClient::new(move || access_token(&tokens));
                    let ctx = SyncContext {
                        library_root: &root,
                        state_path: &state_path,
                        device: &device,
                        now: chrono::Local::now().naive_local(),
                    };
                    rnote_sync::sync(&ctx, &mut drive)
                })
            })
            .await
            .unwrap_or_else(|_| SyncStatus::Failed(gettext("The sync stopped unexpectedly")));
            this.finish(Some(status));
        });
    }

    fn finish(&self, status: Option<SyncStatus>) {
        // Signing out meanwhile wins over what the sync says.
        if let Some(status) = status
            && self.status().signed_in()
        {
            self.set_status(status);
        }
        if self.inner.scheduler.borrow_mut().finished() {
            if self.status().signed_in() {
                self.start();
            } else {
                self.inner.scheduler.borrow_mut().finished();
            }
        }
    }

    /// Signs in through the browser: Google's page opens there, and once the user agrees
    /// the refresh token goes to the keyring and a sync starts. `done` says how it went.
    pub(crate) fn sign_in(&self, window: &Window, done: impl FnOnce(Result<(), String>) + 'static) {
        let Some(client) = self.inner.client.clone() else {
            done(Err(status::status_text(
                &SyncStatus::Unavailable,
                std::time::SystemTime::now(),
            )));
            return;
        };
        let listener = match TcpListener::bind("127.0.0.1:0").and_then(|listener| {
            let port = listener.local_addr()?.port();
            Ok((listener, port))
        }) {
            Ok(bound) => bound,
            Err(error) => {
                done(Err(error.to_string()));
                return;
            }
        };
        let (listener, port) = listener;
        let redirect_uri = format!("http://127.0.0.1:{port}");
        let pkce = oauth::Pkce::new();
        let state = oauth::random_state();
        let url = oauth::authorization_url(&client, &redirect_uri, &pkce, &state);
        // Clicked again while the first sign-in waits: its page again, not a second one.
        let started = self.inner.signing_in.borrow_mut().start(url.as_str());
        let page = started.as_deref().unwrap_or(url.as_str());
        // Without a browser nobody comes back: the wait stops instead of lasting minutes.
        let cancelled = Arc::new(AtomicBool::new(false));
        let no_browser = Arc::clone(&cancelled);
        UriLauncher::new(page).launch(Some(window), gio::Cancellable::NONE, move |result| {
            if let Err(error) = result {
                error!("Opening Google's sign-in page failed: {error:?}");
                no_browser.store(true, Ordering::SeqCst);
            }
        });
        if started.is_some() {
            return;
        }
        let store = Arc::clone(&self.inner.store);
        let access = Arc::clone(&self.inner.tokens);
        let this = self.clone();
        glib::spawn_future_local(async move {
            let signed_in = gio::spawn_blocking(move || -> Result<(), OAuthError> {
                let code =
                    oauth::wait_for_redirect(&listener, &state, SIGN_IN_TIMEOUT, &cancelled)?;
                let tokens = oauth::exchange_code(&client, &code, &pkce, &redirect_uri)?;
                let refresh_token = tokens
                    .refresh_token
                    .ok_or_else(|| anyhow!("Google sent no refresh token"))?;
                store.save(&refresh_token).map_err(OAuthError::Failed)?;
                // Here and not on the main thread: a sync may hold the lock while it
                // renews the token.
                forget_access_token(&access);
                Ok(())
            })
            .await
            .unwrap_or_else(|_| {
                Err(OAuthError::Failed(anyhow!(
                    "Signing in stopped unexpectedly"
                )))
            });
            this.inner.signing_in.borrow_mut().finish();
            match signed_in {
                Ok(()) => {
                    this.set_status(SyncStatus::Waiting);
                    this.sync_now();
                    done(Ok(()));
                }
                Err(error) => done(Err(messages::sign_in_message(&error))),
            }
        });
    }

    /// Signs out: Google forgets the app's access and the keyring the token. Signed out
    /// here even when Google can't be told; `done` gets that error then.
    pub(crate) fn sign_out(&self, done: impl FnOnce(SignOutOutcome) + 'static) {
        let store = Arc::clone(&self.inner.store);
        let access = Arc::clone(&self.inner.tokens);
        let this = self.clone();
        glib::spawn_future_local(async move {
            let outcome = gio::spawn_blocking(move || {
                forget_access_token(&access);
                forget_refresh_token(store.as_ref(), |token| {
                    rnote_sync::revoke(token).map_err(|error| error.to_string())
                })
            })
            .await
            .unwrap_or_else(|_| SignOut::Done(Err("Signing out stopped unexpectedly".to_owned())));
            let revoked = match outcome {
                // The token stays until the keyring opens: this computer is still signed in.
                SignOut::KeyringLocked => {
                    this.set_status(SyncStatus::KeyringLocked);
                    done(SignOutOutcome::KeyringLocked);
                    return;
                }
                SignOut::Done(revoked) => revoked,
            };
            if let Some(source) = this.inner.soon.borrow_mut().take() {
                source.remove();
            }
            this.set_status(SyncStatus::SignedOut);
            done(match revoked {
                Ok(()) => SignOutOutcome::Done,
                Err(error) => {
                    tracing::warn!("Telling Google to forget the sign-in failed: {error}");
                    SignOutOutcome::GoogleNotTold
                }
            });
        });
    }
}

/// How signing out went, for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignOutOutcome {
    /// Signed out, and Google forgot the app's access.
    Done,
    /// Signed out here, but Google couldn't be told (e.g. offline).
    GoogleNotTold,
    /// The keyring is locked: still signed in until it opens.
    KeyringLocked,
}

/// How signing out went.
enum SignOut {
    /// The token is gone from this computer; whether Google was told to forget it too.
    Done(Result<(), String>),
    /// The keyring couldn't be read or changed, so the token is still there.
    KeyringLocked,
}

/// Tells Google to forget the refresh token in `store` with `revoke`, then removes it from
/// `store`; off the main thread.
fn forget_refresh_token(
    store: &dyn RefreshTokenStore,
    revoke: impl FnOnce(&str) -> Result<(), String>,
) -> SignOut {
    let Ok(token) = store.load() else {
        return SignOut::KeyringLocked;
    };
    let revoked = token.as_deref().map_or(Ok(()), revoke);
    if store.delete().is_err() {
        return SignOut::KeyringLocked;
    }
    SignOut::Done(revoked)
}

/// Access tokens renewed with the refresh token in `store`.
fn access_tokens(client: Option<OAuthClient>, store: Arc<dyn RefreshTokenStore>) -> AccessTokens {
    AccessTokens::new(move || {
        let client = client
            .as_ref()
            .ok_or_else(|| anyhow!("No Google Drive credentials"))?;
        let token = store
            .load()
            .map_err(OAuthError::Failed)?
            .ok_or(OAuthError::Revoked)?;
        oauth::refresh(client, &token)
    })
}

/// Drops the access token, waiting for a sync that is renewing it: off the main thread only.
fn forget_access_token(tokens: &Mutex<AccessTokens>) {
    if let Ok(mut tokens) = tokens.lock() {
        tokens.forget();
    }
}

/// The access token Drive gets, in Drive's terms.
fn access_token(tokens: &Mutex<AccessTokens>) -> Result<String, RemoteError> {
    let mut tokens = tokens
        .lock()
        .map_err(|_| RemoteError::Failed(anyhow!("Token lock poisoned")))?;
    tokens.get(Instant::now()).map_err(|error| match error {
        OAuthError::Revoked => RemoteError::Unauthorized,
        OAuthError::Offline(message) => RemoteError::Offline(message),
        other => RemoteError::Failed(anyhow!(other)),
    })
}

/// Whether the network becoming `available` should run a sync now: one waits for Google.
fn retries_on_network(status: &SyncStatus, available: bool) -> bool {
    available && *status == SyncStatus::Pending
}

#[cfg(test)]
mod tests {
    use super::retries_on_network;
    use super::status::SyncStatus;

    #[test]
    fn a_sync_waiting_for_google_runs_when_the_network_comes_back() {
        assert!(retries_on_network(&SyncStatus::Pending, true));
        assert!(!retries_on_network(&SyncStatus::Pending, false));
        assert!(!retries_on_network(&SyncStatus::SignedOut, true));
    }

    #[test]
    fn signing_out_with_the_keyring_locked_doesnt_pretend_it_signed_out() {
        use super::{SignOut, forget_refresh_token};
        use crate::sync::credentials::{MemoryStore, RefreshTokenStore};

        struct Locked;
        impl RefreshTokenStore for Locked {
            fn load(&self) -> anyhow::Result<Option<String>> {
                Err(anyhow::anyhow!("locked"))
            }
            fn save(&self, _token: &str) -> anyhow::Result<()> {
                Err(anyhow::anyhow!("locked"))
            }
            fn delete(&self) -> anyhow::Result<()> {
                Err(anyhow::anyhow!("locked"))
            }
        }
        assert!(matches!(
            forget_refresh_token(&Locked, |_| Ok(())),
            SignOut::KeyringLocked
        ));

        let store = MemoryStore::default();
        store.save("r").unwrap();
        let mut revoked = None;
        let outcome = forget_refresh_token(&store, |token| {
            revoked = Some(token.to_owned());
            Ok(())
        });
        assert!(matches!(outcome, SignOut::Done(Ok(()))));
        assert_eq!(revoked.as_deref(), Some("r"));
        assert_eq!(store.load().unwrap(), None);
    }
}
