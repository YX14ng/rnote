# Etapa 5: sincronización con Google Drive en el Rnote de escritorio — plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** el Rnote de escritorio del fork sincroniza la biblioteca (`/mnt/Storage/HandNotes`)
con la carpeta `Rnote` del Drive de cada usuario, igual que Android, con inicio de sesión
por el navegador y sin perder nunca una versión.

**Architecture:** lo testeable va a Rust sin GTK: `rnote-sync` gana OAuth para apps
instaladas (PKCE, redirección a loopback, canje y renovación de tokens, caché del token de
acceso) y el guardado de cuadernos a prueba de conflictos, que hoy vive en `rnote-android`.
`rnote-ui` agrega un módulo `sync/` (credenciales de compilación, refresh token en el
llavero con `keyring`, planificador, estado y botón) y hace que el guardado de un cuaderno
de la biblioteca pase por el guardado a prueba de conflictos. El Flatpak del fork gana
permiso para el llavero y el script de instalación inyecta las credenciales al compilar.

**Tech Stack:** Rust (edición 2024), `ureq` 3 + rustls, `url`, `sha2`, `base64`, `rand`,
`keyring` 3, GTK4/libadwaita (gtk-rs), gettext, Flatpak (GNOME 50), meson.

**Spec:** `docs/superpowers/specs/2026-11-10-biblioteca-sync-busqueda-design.md`
(secciones "Sincronización con Google Drive" y "Escritorio: cómo llega al usuario").

## Global Constraints

- Scope único: `https://www.googleapis.com/auth/drive.file`. Nunca `drive` ni `drive.readonly`.
- Client id y secret del cliente "Desktop app" entran al compilar por
  `RNOTE_GDRIVE_CLIENT_ID` y `RNOTE_GDRIVE_CLIENT_SECRET`, leídas con `option_env!`; sin
  ellas el botón de sincronizar aparece deshabilitado con un mensaje que lo explica. Nunca
  en el repo ni en un archivo que git vea. En esta PC están en
  `~/.config/rnote-android/desktop-client.json` (modo 600): no imprimirlas nunca.
- OAuth 2.0 para apps instaladas: redirección a `http://127.0.0.1:<puerto>`, PKCE (S256),
  navegador del sistema. Refresh token en el crate `keyring` (Secret Service en Linux,
  incluido Flatpak).
- "Cerrar sesión" revoca el token y lo borra del llavero. Va en el menú del botón de nube,
  como en Android, y no en Preferencias (el spec dice "en los ajustes"; el botón es donde
  ya está en la tablet).
- Nombre de dispositivo para las copias de conflicto: el hostname (`glib::host_name()`).
- Cuándo sincroniza: al abrir la app, al volver a la biblioteca, 5 s después de guardar un
  cuaderno de la biblioteca, con el botón. Sin conexión: "pendiente", sin toasts repetidos,
  reintenta en el siguiente disparador. Una sola sincronización a la vez; un disparador
  durante una sincronización encadena una más al terminar.
- Cuaderno abierto que cambió por debajo: sin cambios sin guardar → se recarga solo con un
  aviso; con cambios → al guardar van a una copia de conflicto y el archivo de abajo no se
  toca.
- Los `.xopp` y lo que esté en carpetas ocultas no se sincronizan (ya lo decide
  `rnote_sync::local::is_synced`).
- Código nuevo: Google Rust Style Guide, mismos `[lints]` de cada crate; upstream no se
  reformatea (`rustfmt --edition 2024` solo sobre archivos nuevos o propios del fork).
- Archivos de 200–400 líneas típico, 800 tope.
- Textos de UI con `gettext`, traducidos en `crates/rnote-ui/po/es.po` y `zh_Hans.po`, y los
  archivos nuevos agregados a `crates/rnote-ui/po/POTFILES`.
- Verificación: `CARGO_HOME=$PWD/_mesonbuild/cargo-home`; `cargo test`, `cargo clippy
  --all-targets -- -D warnings` y `cargo fmt --check` de `rnote-sync`, `rnote-android` y
  `rnote` (paquete de `rnote-ui`); clippy de Android con
  `. android/scripts/env.sh aarch64 && cargo clippy -p rnote-android --target aarch64-linux-android -- -D warnings`.
- PC: no abrir ventanas, menús, navegadores ni apps gráficas (DP3 corre un bot). Compilar e
  instalar el Flatpak sí; lanzar la app y tocar "Iniciar sesión" lo hace el usuario.

## Review Focus

1. Cambiar `library-dir` a otra carpeta y volver: el estado de una biblioteca nunca se usa
   con otra (sería un "nuevo en los dos lados" que llena todo de copias de conflicto).
   → el estado va en un archivo por raíz (Tarea 3, test `state_path_differs_per_library`).
2. El usuario cierra la pestaña del navegador sin autorizar, o el navegador pide
   `/favicon.ico`: la espera no se cuelga para siempre ni toma el pedido equivocado como
   respuesta. → Tarea 2, tests `redirect_ignores_other_paths` y `redirect_times_out`.
3. El refresh token fue revocado desde la cuenta de Google (myaccount): la sincronización
   pide iniciar sesión de nuevo, una vez, sin toasts en cada disparador ni bucles.
   → Tarea 2 (`refresh_reports_a_revoked_grant`) y Tarea 3 (`unauthorized_twice_needs_sign_in`).
4. Guardar un cuaderno de la biblioteca justo cuando la sincronización lo baja: el guardado
   espera el candado de escritura y no pisa la versión bajada.
   → Tarea 1 conserva `write_notebook_waits_for_the_write_guard`.
5. Un cuaderno abierto desde fuera de la biblioteca (Documentos, un pendrive): se guarda como
   siempre, sin hash esperado ni copias de conflicto. → Tarea 5, test
   `library_path_is_none_outside_the_library`.

---

### Tarea 1: el guardado a prueba de conflictos pasa a `rnote-sync`

**Files:**
- Create: `crates/rnote-sync/src/notebooks.rs`, `crates/rnote-sync/src/tests/notebooks.rs`
- Modify: `crates/rnote-sync/src/lib.rs`, `crates/rnote-sync/src/tests/mod.rs`,
  `crates/rnote-android/src/sync_bridge.rs` (pierde `WrittenNotebook`, `write_notebook`,
  `create_notebook`, `check_notebook_path`, `write_atomic`, `sha256_hex` y sus tests),
  `crates/rnote-android/src/jni_bridge/sync.rs`

**Interfaces:**
- Produces (re-exportado desde `lib.rs`):
  - `pub struct WrittenNotebook { pub path: String, pub sha256: String, pub conflict: bool }`
  - `pub fn write_notebook(root: &Path, path: &str, expected_sha256: Option<&str>, device: &str, now: NaiveDateTime, bytes: &[u8]) -> anyhow::Result<WrittenNotebook>`
  - `pub fn create_notebook(root: &Path, path: &str, bytes: &[u8]) -> anyhow::Result<()>`
  - `pub fn sha256_hex(bytes: &[u8]) -> String`

- [ ] **Step 1:** mover los 9 tests `write_notebook_*` / `create_notebook_*` de
  `sync_bridge.rs` a `rnote-sync/src/tests/notebooks.rs` llamando a `crate::write_notebook`
  y `crate::create_notebook`, sin cambiar sus aserciones.
- [ ] **Step 2:** `cargo test -p rnote-sync notebooks` → no compila (funciones inexistentes).
- [ ] **Step 3:** mover las funciones tal cual a `notebooks.rs` (con `pub`), re-exportarlas;
  `rnote-android` las usa desde `rnote_sync`.
- [ ] **Step 4:** `cargo test -p rnote-sync` y `cargo test -p rnote-android` verdes (mismo
  total de tests entre los dos que antes); clippy host y aarch64 limpios.
- [ ] **Step 5:** commit `refactor: el guardado a prueba de conflictos pasa a rnote-sync para usarlo en el escritorio`

### Tarea 2: OAuth para apps instaladas en `rnote-sync`

**Files:**
- Create: `crates/rnote-sync/src/oauth.rs`, `crates/rnote-sync/src/tests/oauth.rs`
- Modify: `crates/rnote-sync/src/lib.rs`, `crates/rnote-sync/src/tests/mod.rs`,
  `crates/rnote-sync/Cargo.toml` (`base64`, `rand`)

**Interfaces:**
- Produces (`pub mod oauth`):
  - `pub const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";`
  - `pub struct OAuthClient { pub id: String, pub secret: String }`
  - `pub struct Pkce { pub verifier: String, pub challenge: String }` con
    `Pkce::new() -> Pkce` (32 bytes al azar en base64url sin relleno) y
    `Pkce::from_verifier(verifier: &str) -> Pkce`
  - `pub fn random_state() -> String` (16 bytes al azar, base64url)
  - `pub fn authorization_url(client: &OAuthClient, redirect_uri: &str, pkce: &Pkce, state: &str) -> url::Url`
    contra `https://accounts.google.com/o/oauth2/v2/auth` con `client_id`, `redirect_uri`,
    `response_type=code`, `scope=SCOPE`, `code_challenge`, `code_challenge_method=S256`,
    `state`, `access_type=offline`, `prompt=consent`
  - `pub fn wait_for_redirect(listener: &TcpListener, state: &str, timeout: Duration) -> Result<String, OAuthError>`:
    atiende pedidos hasta que llega uno a `/` con `state` igual (devuelve `code`) o con
    `error` (→ `Denied`); a otras rutas responde 404 y sigue; contesta al navegador una
    página 200 corta "Listo: ya podés volver a Rnote. / Done: you can go back to Rnote."
  - `pub struct Tokens { pub access_token: String, pub expires_in: Duration, pub refresh_token: Option<String> }`
  - `pub fn exchange_code_at(base_url: &str, client: &OAuthClient, code: &str, pkce: &Pkce, redirect_uri: &str) -> Result<Tokens, OAuthError>`
    (`POST {base}/token`, form: `code`, `client_id`, `client_secret`, `code_verifier`,
    `grant_type=authorization_code`, `redirect_uri`) y `exchange_code` contra
    `https://oauth2.googleapis.com`
  - `pub fn refresh_at(base_url: &str, client: &OAuthClient, refresh_token: &str) -> Result<Tokens, OAuthError>`
    (`grant_type=refresh_token`) y `refresh`
  - `pub enum OAuthError { Denied, StateMismatch, TimedOut, Revoked, Offline(String), Failed(anyhow::Error) }`
    (`thiserror`); `400` con `{"error":"invalid_grant"}` → `Revoked`; errores de transporte
    → `Offline` (reusar `drive::transport_error`)
  - `pub struct AccessTokens` con `new(refresh: impl FnMut() -> Result<Tokens, OAuthError> + Send + 'static)`,
    `get(&mut self, now: Instant) -> Result<String, OAuthError>` (renueva si no hay o vence
    en menos de 60 s) y `forget(&mut self)`

- [ ] **Step 1: tests** en `tests/oauth.rs`:
  - `pkce_matches_rfc_7636`: `Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk").challenge == "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"`; `Pkce::new().verifier.len() == 43`.
  - `authorization_url_asks_only_for_drive_file`: los 9 parámetros de arriba con sus valores; `scope` es exactamente `SCOPE`.
  - `redirect_returns_the_code`: un `TcpStream` manda `GET /?state=s&code=c HTTP/1.1` → `Ok("c")` y la respuesta empieza con `HTTP/1.1 200`.
  - `redirect_ignores_other_paths`: primero `GET /favicon.ico` (recibe 404), después el bueno → `Ok`.
  - `redirect_rejects_another_state`: `state=x` → `Err(StateMismatch)`.
  - `redirect_reports_a_denial`: `GET /?state=s&error=access_denied` → `Err(Denied)`.
  - `redirect_times_out`: nadie se conecta, timeout 200 ms → `Err(TimedOut)`.
  - `code_exchange_sends_the_verifier`: stub 200 `{"access_token":"a","expires_in":3599,"refresh_token":"r"}` → `Tokens` con esos valores; el pedido es `POST /token` con `code_verifier` y `grant_type=authorization_code`.
  - `refresh_reports_a_revoked_grant`: stub 400 `{"error":"invalid_grant"}` → `Err(Revoked)`.
  - `refresh_offline`: base a un puerto cerrado → `Err(Offline(_))`.
  - `access_tokens_are_reused_until_a_minute_before_expiry`: `expires_in` 3600 s; `get(t0)`, `get(t0+3500 s)` → una sola renovación; `get(t0+3541 s)` → dos.
  - `forgotten_access_token_is_renewed`.
- [ ] **Step 2:** `cargo test -p rnote-sync oauth` → no compila.
- [ ] **Step 3:** implementar en `oauth.rs`. La espera: `listener.set_nonblocking(true)` y
  sondeo cada 50 ms hasta el timeout; cada conexión se lee hasta la línea en blanco.
- [ ] **Step 4:** `cargo test -p rnote-sync` verde; clippy y fmt limpios.
- [ ] **Step 5:** commit `feat: OAuth con PKCE y redirección local en rnote-sync para el escritorio`

### Tarea 3: núcleo de la sincronización de escritorio

**Files:**
- Create: `crates/rnote-ui/src/sync/mod.rs` (`RnSync`, el manejador), `sync/credentials.rs`,
  `sync/schedule.rs`, `sync/status.rs`
- Modify: `crates/rnote-ui/Cargo.toml` (`rnote-sync`, `keyring = { version = "3", features = ["sync-secret-service", "crypto-rust", "windows-native", "apple-native"] }`, en `[workspace.dependencies]` del `Cargo.toml` raíz), `crates/rnote-ui/src/main.rs` (declara `mod sync;`), `crates/rnote-ui/src/appwindow/imp.rs` (el `RnSync` vive en la ventana)

**Interfaces:**
- Consumes: Tarea 2 (`oauth::*`), `rnote_sync::{sync, SyncContext, SyncError, SyncReport, DriveClient, revoke}`.
- Produces:
  - `credentials::build_client() -> Option<OAuthClient>` (desde `option_env!`; vacío cuenta como ausente)
  - `credentials::RefreshTokenStore` (trait: `load`, `save`, `delete` → `anyhow::Result`), con
    `KeyringStore` (servicio = app id de la config de compilación, usuario `"google-drive"`)
  - `schedule::Scheduler` con `request(&mut self) -> bool` (true = arrancar ya; false = queda
    encolada) y `finished(&mut self) -> bool` (true = arrancar la encolada)
  - `schedule::state_path(data_dir: &Path, library_root: &Path) -> PathBuf` =
    `data_dir/rnote/sync/<16 primeros hex de sha256(raíz canónica)>.json`
  - `status::SyncStatus { Unavailable, SignedOut, SignInNeeded, Running, Done { at: SystemTime, report: SyncReport }, Pending, Failed(String) }`
  - `status::Ago { JustNow, Minutes(u64), Hours(u64), OnDate }` con `Ago::of(elapsed: Duration)`
    (mismos cortes que `SyncAgo` de Android: < 1 min, < 1 h, < 24 h)
  - `status::status_text(status: &SyncStatus, now: SystemTime) -> String` (gettext)
  - `RnSync`: `status()`, `connect_status(f)`, `sync_now()`, `sync_soon()` (5 s, se reinicia
    con cada llamada), `sign_in(window: &gtk::Window)`, `sign_out()`; corre la
    sincronización en `gio::spawn_blocking` con un `DriveClient` cuyo token sale de
    `AccessTokens`. `Unauthorized` → `forget()` y un reintento; si vuelve o la renovación da
    `Revoked` → borra el refresh token y queda `SignInNeeded`.

- [ ] **Step 1: tests** (`cargo test -p rnote sync::`):
  - `a_request_while_running_is_chained_once`: `request()` true; dos `request()` false;
    `finished()` true (una sola encadenada); `finished()` false.
  - `state_path_differs_per_library`: dos raíces → dos rutas distintas bajo `data_dir/rnote/sync/`, terminadas en `.json`.
  - `ago_steps`: 0 s y 59 s → `JustNow`; 60 s → `Minutes(1)`; 3599 s → `Minutes(59)`; 3600 s → `Hours(1)`; 86400 s → `OnDate`.
  - `pending_says_offline_not_the_error`: `status_text(Pending)` no contiene `"io:"` y contiene `"Offline"`.
  - `unauthorized_twice_needs_sign_in`: con un `RefreshTokenStore` en memoria y una
    función de sincronización falsa que devuelve `Unauthorized` dos veces, el resultado es
    `SignInNeeded` y el store quedó vacío (la lógica de reintento va en una función pura
    `run_with_retry(tokens, store, attempt) -> SyncStatus` que `RnSync` llama).
- [ ] **Step 2:** `cargo test -p rnote sync::` → no compila.
- [ ] **Step 3:** implementar. El inicio de sesión: `TcpListener::bind("127.0.0.1:0")`,
  `authorization_url`, `gtk::UriLauncher::new(url).launch(Some(window), …)` (portal en
  Flatpak), `wait_for_redirect` (timeout 5 min) y `exchange_code` en `spawn_blocking`,
  `save` del refresh token, y `sync_now()`. Cerrar sesión: `revoke` del refresh token (si
  falla por red, igual se borra local y se avisa), `delete`, `SignedOut`.
- [ ] **Step 4:** tests verdes; clippy y fmt limpios.
- [ ] **Step 5:** commit `feat: sincronización de escritorio con inicio de sesión por el navegador y token en el llavero`

### Tarea 4: botón, disparadores y traducciones

**Files:**
- Create: `crates/rnote-ui/src/sync/button.rs`
- Modify: `crates/rnote-ui/data/ui/library/libraryview.ui` (botón en la barra de la
  biblioteca), `crates/rnote-ui/src/library/mod.rs`, `crates/rnote-ui/src/appwindow/library.rs`
  (`show_library` → `sync_now()`), arranque de la ventana (`sync_now()` al abrir),
  `crates/rnote-ui/src/canvas/imexport.rs` (tras guardar un cuaderno de la biblioteca →
  `sync_soon()`), `crates/rnote-ui/po/POTFILES`, `po/es.po`, `po/zh_Hans.po`

**Interfaces:**
- Consumes: `RnSync` de la Tarea 3.
- Produces: `button::SyncButton` (`gtk::MenuButton` con ícono simbólico de nube según el
  estado: `Unavailable` deshabilitado con tooltip que lo explica; `SignedOut`/`SignInNeeded`
  → un clic inicia sesión; si no, popover con el texto de estado, "Sincronizar ahora" y
  "Cerrar sesión de Google Drive" con confirmación). Un toast por error distinto (no por
  intento); un toast por copias de conflicto con su cantidad; `Done` con cambios recarga la
  biblioteca si está visible.

- [ ] **Step 1:** test `sync_button_icon_per_status` (función pura
  `button::icon_name(&SyncStatus) -> &'static str`: nube tachada para `Unavailable`,
  `SignedOut`, `SignInNeeded`; nube con flechas para `Running`; nube para `Done`; nube
  con signo para `Pending` y `Failed`).
- [ ] **Step 2:** `cargo test -p rnote sync::button` → falla.
- [ ] **Step 3:** implementar, cablear disparadores y traducir los textos nuevos.
- [ ] **Step 4:** tests de `rnote` verdes; `meson compile -C _mesonbuild` compila (valida el
  `.ui`); clippy limpio.
- [ ] **Step 5:** commit `feat: botón de Google Drive en la biblioteca de escritorio y sincronización al abrir, al volver y al guardar`

### Tarea 5: guardar sin pisar y recargar lo que cambió por debajo

**Files:**
- Modify: `crates/rnote-ui/src/canvas/mod.rs` (campo `library_sha256: RefCell<Option<String>>`),
  `crates/rnote-ui/src/canvas/imexport.rs` (`load_in_rnote_bytes` y
  `save_document_to_file`), el watcher de `canvas/mod.rs` (~l. 1070–1160)
- Create: `crates/rnote-ui/src/sync/opened.rs`

**Interfaces:**
- Consumes: `rnote_sync::{write_notebook, sha256_hex}`, `RnSync::sync_soon`.
- Produces:
  - `opened::library_path(root: &Path, file: &Path) -> Option<String>` (ruta relativa con
    `/` si el archivo es un `.rnote` dentro de la raíz y fuera de carpetas ocultas)
  - `opened::on_changed_on_disk(unsaved: bool, in_library: bool) -> Reaction { Reload, AskAsBefore }`

- [ ] **Step 1: tests:**
  - `library_path_inside` (`root/Algebra/a.rnote` → `Some("Algebra/a.rnote")`),
  - `library_path_is_none_outside_the_library` (otra carpeta, `.xopp`, `.rnote-library/…`),
  - `changed_on_disk_reloads_only_clean_library_notebooks` (solo `(false, true)` → `Reload`).
- [ ] **Step 2:** `cargo test -p rnote sync::opened` → no compila.
- [ ] **Step 3:** implementar. Al cargar un cuaderno de la biblioteca se guarda el sha256
  de los bytes leídos. Al guardar uno de la biblioteca, en lugar de
  `atomic_save_to_file_future` se llama a `write_notebook` (en `spawn_blocking`, con
  `set_output_file_expect_write(true)` alrededor como hoy); se actualiza `library_sha256`;
  si `conflict`, el `output_file` pasa a la copia y un toast dice "Guardado como copia de
  conflicto: <nombre>". Fuera de la biblioteca, todo como antes. El watcher: `Reload` →
  `reload_from_disk()` con toast "Actualizado desde otro dispositivo"; `AskAsBefore` → el
  toast de siempre.
- [ ] **Step 4:** tests de `rnote` verdes, clippy limpio, `meson compile` compila.
- [ ] **Step 5:** commit `feat: el escritorio no pisa un cuaderno que cambió por debajo y recarga los que no tienen cambios`

### Tarea 6: Flatpak con llavero y credenciales

**Files:**
- Modify: `build-aux/io.github.yx14ng.rnote.json` (finish-arg `--talk-name=org.freedesktop.secrets`),
  `build-aux/install-fork-flatpak.sh`, `docs/fork-desktop.md`

**Interfaces:**
- El script toma `RNOTE_GDRIVE_CLIENT_ID`/`RNOTE_GDRIVE_CLIENT_SECRET` del entorno o, si
  faltan y existe `RNOTE_GDRIVE_CLIENT_JSON` (por defecto
  `~/.config/rnote-android/desktop-client.json`), de `installed.client_id` /
  `installed.client_secret`; escribe una copia del manifiesto con `build-options.env` y la
  fuente con ruta absoluta en un `mktemp -d` fuera del repo (borrado con `trap`), y compila
  con esa copia. Sin credenciales compila igual y avisa que la sincronización queda
  deshabilitada.

- [ ] **Step 1:** `bash -n` y `shellcheck` del script limpios; después de correrlo,
  `git status --short` no muestra archivos nuevos y `git grep -F "$RNOTE_GDRIVE_CLIENT_ID"`
  no encuentra nada (sin imprimir el valor: solo el código de salida).
- [ ] **Step 2:** compilar e instalar: `RNOTE_LIBRARY_DIR=/mnt/Storage/HandNotes build-aux/install-fork-flatpak.sh`
  → instala `io.github.yx14ng.rnote`; `flatpak info --show-permissions` incluye
  `org.freedesktop.secrets`.
- [ ] **Step 3:** commit `build: el Flatpak del fork accede al llavero y compila con las credenciales de Drive` y
  `docs: cómo compilar el escritorio con la sincronización de Drive` (dos commits).

### Tarea 7: prueba de punta a punta (con el usuario)

- [ ] **Step 1:** el usuario abre "Rnote (YX14ng)" y toca el botón de nube → se abre su
  navegador, autoriza con la misma cuenta que la tablet → el botón dice "Sincronizado hace
  un momento"; los cuadernos de la tablet aparecen en HandNotes y los de HandNotes en la
  tablet.
- [ ] **Step 2:** escribir en la PC y guardar → a los segundos está en la tablet; escribir en
  la tablet → la PC lo baja y, con el cuaderno abierto sin cambios, lo recarga con aviso.
- [ ] **Step 3:** el mismo cuaderno editado en los dos sin sincronizar → copia de conflicto
  en los dos lados, nada perdido.
- [ ] **Step 4:** cortar la red de la app y sincronizar → "pendiente" sin toasts repetidos.
- [ ] **Step 5:** "Cerrar sesión de Google Drive" → el botón pide iniciar sesión y el
  llavero ya no tiene la entrada (`secret-tool search service io.github.yx14ng.rnote`).
- [ ] **Step 6:** anotar cada resultado en el ledger; push de la rama al cerrar.
