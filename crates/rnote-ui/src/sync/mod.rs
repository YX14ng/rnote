//! Syncing the library with Google Drive from the desktop: signing in through the
//! browser, the refresh token in the keyring, and one sync at a time on a worker thread.

pub(crate) mod button;
pub(crate) mod credentials;
pub(crate) mod run;
pub(crate) mod schedule;
pub(crate) mod status;

use std::cell::RefCell;
use std::net::TcpListener;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use gtk4::{UriLauncher, Window, gio, glib};
use rnote_sync::oauth::{self, AccessTokens, OAuthClient, OAuthError};
use rnote_sync::{DriveClient, RemoteError, SyncContext};
use tracing::{error, warn};

use credentials::{KeyringStore, RefreshTokenStore};
use schedule::Scheduler;
pub(crate) use status::SyncStatus;

/// How long after a save [`RnSync::sync_soon`] waits.
const SOON_DELAY: Duration = Duration::from_secs(5);
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
    listeners: RefCell<Vec<Listener>>,
    root: RefCell<Option<RootProvider>>,
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
        let status = match (&client, store.load()) {
            (None, _) => SyncStatus::Unavailable,
            (Some(_), Ok(Some(_))) => SyncStatus::Waiting,
            (Some(_), Ok(None)) => SyncStatus::SignedOut,
            (Some(_), Err(error)) => {
                warn!("Reading the Google Drive token from the keyring failed: {error:?}");
                SyncStatus::SignedOut
            }
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
                listeners: RefCell::new(Vec::new()),
                root: RefCell::new(None),
            }),
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
            .unwrap_or_else(|_| SyncStatus::Failed("The sync stopped unexpectedly".to_owned()));
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
        UriLauncher::new(url.as_str()).launch(Some(window), gio::Cancellable::NONE, |result| {
            if let Err(error) = result {
                error!("Opening Google's sign-in page failed: {error:?}");
            }
        });
        let store = Arc::clone(&self.inner.store);
        let this = self.clone();
        glib::spawn_future_local(async move {
            let signed_in = gio::spawn_blocking(move || -> Result<(), OAuthError> {
                let code = oauth::wait_for_redirect(&listener, &state, SIGN_IN_TIMEOUT)?;
                let tokens = oauth::exchange_code(&client, &code, &pkce, &redirect_uri)?;
                let refresh_token = tokens
                    .refresh_token
                    .ok_or_else(|| anyhow!("Google sent no refresh token"))?;
                store.save(&refresh_token).map_err(OAuthError::Failed)?;
                Ok(())
            })
            .await
            .unwrap_or_else(|_| {
                Err(OAuthError::Failed(anyhow!(
                    "Signing in stopped unexpectedly"
                )))
            });
            match signed_in {
                Ok(()) => {
                    if let Ok(mut tokens) = this.inner.tokens.lock() {
                        tokens.forget();
                    }
                    this.set_status(SyncStatus::Waiting);
                    this.sync_now();
                    done(Ok(()));
                }
                Err(error) => done(Err(error.to_string())),
            }
        });
    }

    /// Signs out: Google forgets the app's access and the keyring the token. Signed out
    /// here even when Google can't be told; `done` gets that error then.
    pub(crate) fn sign_out(&self, done: impl FnOnce(Result<(), String>) + 'static) {
        let store = Arc::clone(&self.inner.store);
        let this = self.clone();
        glib::spawn_future_local(async move {
            let revoked = gio::spawn_blocking(move || -> Result<(), String> {
                let token = store.load().map_err(|error| error.to_string())?;
                let revoked = match token {
                    Some(token) => rnote_sync::revoke(&token).map_err(|error| error.to_string()),
                    None => Ok(()),
                };
                store.delete().map_err(|error| error.to_string())?;
                revoked
            })
            .await
            .unwrap_or_else(|_| Err("Signing out stopped unexpectedly".to_owned()));
            if let Ok(mut tokens) = this.inner.tokens.lock() {
                tokens.forget();
            }
            if let Some(source) = this.inner.soon.borrow_mut().take() {
                source.remove();
            }
            this.set_status(SyncStatus::SignedOut);
            done(revoked);
        });
    }
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
