# Etapa 4: sincronización con Google Drive en Android — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** En la tablet, un botón "Iniciar sesión con Google" en la biblioteca y, una vez
autorizada, la sincronización de toda la biblioteca con la carpeta `Rnote` del Drive del
usuario: al abrir la app y volver a la biblioteca, unos segundos después de guardar, y
con el botón; sin pisar nunca un cuaderno abierto en el editor.

**Architecture:** Rust hace todo lo que se puede testear: `rnote-sync` gana la revocación
del token y el nombre de copia de conflicto público; `rnote-android` gana un puente de
sincronización (JSON de resultado, testeado con el Drive falso) y la escritura protegida
de cuadernos de la biblioteca (si el archivo cambió desde que el editor lo leyó, guarda una
copia de conflicto al lado). Kotlin solo obtiene el token con la Authorization API de Play
Services, decide cuándo sincronizar (`SyncManager`, un hilo propio) y lo muestra (botón de
nube en la barra de la biblioteca, recarga del cuaderno abierto).

**Tech Stack:** Rust 2024 del workspace; `rnote-sync` (ureq 3 + rustls/ring);
`jni` 0.22; Kotlin sin AndroidX propio; `com.google.android.gms:play-services-auth`
(22.0.0, o 21.6.0 si 22 no compila); Gradle/AGP del proyecto (AGP 9.4.1, compileSdk 36).

**Spec:** `docs/superpowers/specs/2026-11-10-biblioteca-sync-busqueda-design.md`
(secciones "Sincronización con Google Drive": "Distribución pública", "Autorización",
"Algoritmo" (nombre de dispositivo), "Cuándo sincroniza"; "Anti-objetivos";
"Verificación"; "Orden propuesto" etapa 4).

## Global Constraints

- Scope único `https://www.googleapis.com/auth/drive.file`. Ningún client id, secreto ni
  token en el repo, en los logs ni en los tests (los tests usan `"token-1"`).
- El cliente OAuth de Android se identifica por paquete `io.github.yx14ng.rnote.viewer` +
  SHA-1 de la clave estable: el código no lleva client id.
- Builds sin la clave estable (`RNOTE_KEYSTORE` sin definir): la sincronización aparece
  deshabilitada con una explicación, nunca falla con un error críptico.
- Nombre de dispositivo para las copias de conflicto: `Build.MODEL`.
- Estado de sincronización fuera de la biblioteca: `filesDir/sync/state.json`
  (la biblioteca es `filesDir/library`).
- Anti-objetivo central: ninguna sincronización ni guardado pisa una versión sin dejar copia.
- Sin AndroidX en el código de la app (las dependencias transitivas de Play Services sí lo
  traen: `android.useAndroidX=true`); las Activities siguen siendo `android.app.Activity`.
- Rust nuevo: Google Rust Style Guide, mismos `[lints]` de cada crate, tests como módulos;
  upstream no se reformatea. Kotlin nuevo con el estilo de los archivos vecinos.
- Archivos de 200–400 líneas típico, 800 tope; `MainActivity.kt` (503) y
  `LibraryActivity.kt` (319) solo reciben el cableado, la lógica va a archivos nuevos.
- Textos de UI en `values/strings.xml` (inglés) y `values-es/strings.xml` (español).
- Tablet: chequear `topResumedActivity` en un comando aparte justo antes de cada
  lanzamiento; no lanzar si el usuario está en otra app.

## Review Focus

1. Cuaderno abierto y modificado en el editor mientras la sincronización baja una versión
   nueva del mismo archivo: al guardar, los cambios del editor van a una copia de conflicto
   y la versión bajada queda intacta (test Rust `write_notebook_*` en la tarea 2; prueba en
   la tablet en la tarea 7).
2. El token vence a mitad de camino (Drive responde 401): se descarta el token, se pide uno
   nuevo sin interfaz y se reintenta una sola vez; si vuelve a fallar, el botón pide iniciar
   sesión, sin ventanas que aparezcan solas (tarea 5).
3. Sin conexión: la sincronización queda "pendiente", el botón lo muestra, no hay un toast
   por cada intento, y el próximo disparador reintenta (test `offline_is_reported` en la
   tarea 3; tarea 5).
4. Disparadores superpuestos (volver a la biblioteca justo después del guardado con
   retardo): nunca dos sincronizaciones a la vez; la segunda se encadena al terminar la
   primera (tarea 5; el candado de `rnote-sync` responde `busy` como red de seguridad,
   test `busy_is_reported` en la tarea 3).
5. Usuario sin sesión iniciada, o build sin la clave estable: los disparadores automáticos
   no hacen nada ni muestran nada; solo el botón explica o pide iniciar sesión (tareas 5 y 6).

La app Kotlin no tiene harness de tests unitarios (sin AndroidX ni Robolectric): todo lo
testeable vive en Rust; lo de Kotlin se verifica con build, lint y la prueba en la tablet
de la tarea 7, que cubre los cinco puntos.

---

### Tarea 1: revocar el token y nombrar copias de conflicto desde afuera de `rnote-sync`

**Files:**
- Create: `crates/rnote-sync/src/revoke.rs`
- Modify: `crates/rnote-sync/src/lib.rs` (módulo y re-exports), `crates/rnote-sync/src/names.rs`
- Test: `crates/rnote-sync/src/tests/revoke.rs`, `crates/rnote-sync/src/tests/mod.rs`

**Interfaces:**
- Produces:
  - `pub fn revoke(token: &str) -> Result<(), RemoteError>` — POST a
    `https://oauth2.googleapis.com/revoke`.
  - `pub fn revoke_at(base_url: &str, token: &str) -> Result<(), RemoteError>` — lo mismo
    contra `{base_url}/revoke` (tests).
  - `pub fn conflict_copy_path(library_root: &Path, path: &str, device: &str, now: NaiveDateTime) -> String`
    — `conflict_name(path, device, now)` pasado por `free_path` contra lo que existe en
    disco; es el mismo nombre que da la sincronización.

- [ ] **Step 1: tests que fallan** en `src/tests/revoke.rs` (con `Stub`, como `drive.rs`):
  - `revoke_posts_the_token_as_a_form`: stub `(200, JSON, b"{}")`; `revoke_at(url, "token-1")`
    es `Ok`; el request es `POST /revoke`, `Content-Type` `application/x-www-form-urlencoded`,
    cuerpo `token=token-1`.
  - `revoke_of_an_invalid_token_is_ok`: stub `(400, JSON, {"error":"invalid_token"})` → `Ok(())`
    (ya vencido o revocado: no queda nada que revocar).
  - `revoke_server_error_fails`: stub `(500, …)` → `Err(RemoteError::Failed(_))`.
  - `revoke_unreachable_is_offline`: `revoke_at("http://127.0.0.1:9", …)` → `Err(RemoteError::Offline(_))`.
  - `conflict_copy_path_skips_taken_names`: en un tempdir con `a.rnote` y
    `a (conflicto, Tab, 2026-11-11 10.00).rnote`, `conflict_copy_path(root, "a.rnote", "Tab", 2026-11-11 10:00)`
    es `"a (conflicto, Tab, 2026-11-11 10.00) (2).rnote"`; en carpeta (`"Orga/a.rnote"`)
    conserva la carpeta.
- [ ] **Step 2:** `cargo test -p rnote-sync revoke conflict_copy_path` → no compila (funciones
  inexistentes).
- [ ] **Step 3:** implementar `revoke.rs` con un `Agent` como el de `DriveClient::new`
  (sin seguir errores HTTP como `Err`); el token va URL-encodeado con `url::form_urlencoded`.
  Errores de transporte → `Offline` con la misma clasificación que `drive.rs` (extraer la
  función de clasificación a `pub(crate)` si hoy es privada, sin cambiar su comportamiento).
  `conflict_copy_path` en `names.rs`, `pub` y re-exportada desde `lib.rs`.
- [ ] **Step 4:** `cargo test -p rnote-sync` → todos pasan (86 + 5).
  `cargo clippy -p rnote-sync --all-targets -- -D warnings` limpio.
- [ ] **Step 5: commit** `feat: revocar el token de Drive y nombrar copias de conflicto desde afuera`

### Tarea 2: puente de sincronización y escritura protegida en `rnote-android`

**Files:**
- Create: `crates/rnote-android/src/sync_bridge.rs`, `crates/rnote-android/src/jni_bridge/sync.rs`
- Modify: `crates/rnote-android/Cargo.toml` (deps `rnote-sync`, `chrono`; dev-dep
  `rnote-sync` con `features = ["fake"]`), `crates/rnote-android/src/lib.rs`,
  `crates/rnote-android/src/jni_bridge/mod.rs`,
  `android/viewer/app/src/main/kotlin/io/github/yx14ng/rnote/viewer/NativeBridge.kt`

**Interfaces:**
- Consumes: `rnote_sync::{sync, SyncContext, SyncError, DriveClient, Remote, revoke, conflict_copy_path}`.
- Produces (Rust):
  - `pub(crate) fn run_sync(root: &Path, state_path: &Path, device: &str, remote: &mut dyn Remote, now: NaiveDateTime) -> String`
    — JSON `{"status": "ok", "uploaded", "downloaded", "moved", "deleted", "conflicts": [..], "changed_here": [..], "skipped": [..]}`,
    o `{"status": "busy"}`, `{"status": "unauthorized"}`,
    `{"status": "offline", "message": ..}`, `{"status": "failed", "message": ..}`.
    Nunca devuelve `Err`: el estado viaja en el JSON.
  - `pub(crate) fn write_notebook(root: &Path, path: &str, expected_sha256: Option<&str>, device: &str, now: NaiveDateTime, bytes: &[u8]) -> anyhow::Result<WrittenNotebook>`
    con `pub(crate) struct WrittenNotebook { pub path: String, pub sha256: String, pub conflict: bool }`.
    Si el archivo en `path` existe y su SHA-256 no es `expected_sha256` (o `expected_sha256`
    es `None` y el archivo existe con contenido), escribe en `conflict_copy_path(...)` y
    `conflict: true`; si no, escribe en `path`. Escritura atómica (temporal oculto en la
    misma carpeta + rename). Rechaza `path` fuera de la biblioteca (absoluto, `..`, oculto).
- Produces (JNI / Kotlin `NativeBridge`):
  - `@JvmStatic external fun syncRun(root: String, statePath: String, device: String, token: String): String`
    → `run_sync` con `DriveClient::new(move || Ok(token.clone()))` y `Local::now().naive_local()`.
  - `@JvmStatic external fun syncRevoke(token: String)` → `revoke`; lanza
    `RuntimeException` si falla.
  - `@JvmStatic external fun libraryWriteNotebook(root: String, path: String, expectedSha256: String?, device: String, bytes: ByteArray): String`
    → `write_notebook`; devuelve `{"path", "sha256", "conflict"}`; lanza si falla.

- [ ] **Step 1: tests que fallan** (módulo `#[cfg(test)]` en `sync_bridge.rs`, con `FakeDrive` y tempdirs):
  - `ok_reports_counts_and_changed_paths`: Drive falso con `"a.rnote"`; biblioteca vacía →
    `status == "ok"`, `downloaded == 1`, `changed_here == ["a.rnote"]`.
  - `busy_is_reported`: con el candado tomado (un `sync` en curso simulado tomando
    `<state>.lock` con `File::try_lock` antes) → `{"status": "busy"}`.
  - `offline_is_reported`: `FakeDrive::fail_after(0)` → `status == "offline"` con `message` no vacío.
  - `unauthorized_is_reported`: un `Remote` de test cuyo `list` devuelve
    `RemoteError::Unauthorized` → `{"status": "unauthorized"}`.
  - `write_notebook_writes_when_unchanged`: archivo con hash conocido → escribe en `path`,
    `conflict == false`, `sha256` es el de los bytes nuevos.
  - `write_notebook_keeps_a_changed_file`: archivo cuyo contenido cambió desde
    `expected_sha256` → el archivo original queda igual, los bytes van a
    `"a (conflicto, Tab, 2026-11-11 10.00).rnote"`, `conflict == true`.
  - `write_notebook_creates_a_missing_file`: `path` inexistente → escribe en `path` (un
    cuaderno que la sincronización sacó y el editor guarda vuelve a existir: gana lo modificado).
  - `write_notebook_rejects_paths_outside`: `"../x.rnote"`, `"/x.rnote"`, `".rnote-library/x.rnote"` → `Err`.
- [ ] **Step 2:** `cargo test -p rnote-android sync_bridge` → no compila.
- [ ] **Step 3:** implementar `sync_bridge.rs` y las tres entradas JNI en
  `jni_bridge/sync.rs` (mismo patrón que `jni_bridge/library.rs`:
  `with_env` + `ThrowRuntimeExAndDefault`); declarar las tres en `NativeBridge.kt` con su
  comentario de una línea, como las vecinas.
- [ ] **Step 4:** `cargo test -p rnote-android` → pasan;
  `cargo clippy -p rnote-android --all-targets -- -D warnings` limpio en host y
  `bash -c '. android/scripts/env.sh aarch64 && cargo clippy -p rnote-android --target aarch64-linux-android -- -D warnings'`
  limpio; `cargo tree -p rnote-android -i aws-lc-rs` no encuentra el paquete.
- [ ] **Step 5: commit** `feat: puente JNI de sincronización y guardado protegido de cuadernos`

### Tarea 3: Play Services, permisos y el interruptor de builds sin clave

**Files:**
- Modify: `android/viewer/app/build.gradle.kts`, `android/viewer/gradle.properties`,
  `android/viewer/app/src/main/AndroidManifest.xml`

**Interfaces:**
- Produces: `BuildConfig.DRIVE_SYNC: Boolean` — `true` solo si el build se firma con la
  clave estable (`RNOTE_KEYSTORE` definida). Dependencia
  `com.google.android.gms:play-services-auth`. Permiso `android.permission.INTERNET`.

- [ ] **Step 0:** conteo base de lint antes de tocar nada:
  `cd android/viewer && ./gradlew --no-daemon lintDebug`, anotar en el ledger la cantidad de
  avisos de `app/build/reports/lint-results-debug.txt` (o el informe que genere).
- [ ] **Step 1:** `buildFeatures { buildConfig = true }`,
  `buildConfigField("boolean", "DRIVE_SYNC", (stableKeystore != null).toString())` en
  `defaultConfig`; `dependencies { implementation("com.google.android.gms:play-services-auth:22.0.0") }`;
  `android.useAndroidX=true` en `gradle.properties` (con un comentario: lo exige Play
  Services, la app no usa AndroidX); `<uses-permission android:name="android.permission.INTERNET" />`.
- [ ] **Step 2:** `android/scripts/build-apk.sh aarch64` → `BUILD SUCCESSFUL`; luego
  `$ANDROID_HOME/build-tools/*/aapt2 dump permissions app/build/outputs/apk/debug/app-debug.apk`
  lista `android.permission.INTERNET`; `grep DRIVE_SYNC` en
  `app/build/generated/source/buildConfig/debug/**/BuildConfig.java` muestra `true`.
  Si 22.0.0 no resuelve o no compila, 21.6.0, y anotarlo.
- [ ] **Step 3: commit** `chore: Play Services de autorización, permiso de red y BuildConfig.DRIVE_SYNC`

### Tarea 4: `DriveAuth`, el token de la Authorization API

**Files:**
- Create: `android/viewer/app/src/main/kotlin/io/github/yx14ng/rnote/viewer/DriveAuth.kt` (~120 líneas)

**Interfaces:**
- Produces:
  ```kotlin
  internal sealed interface TokenResult {
      data class Token(val value: String) : TokenResult
      /** Hace falta que el usuario elija la cuenta o acepte: solo desde el botón. */
      data class NeedsUser(val intent: PendingIntent) : TokenResult
      /** El build no está registrado en Google (ApiException DEVELOPER_ERROR, código 10). */
      data object NotRegistered : TokenResult
      data class Failed(val error: Throwable) : TokenResult
  }
  internal object DriveAuth {
      const val SCOPE = "https://www.googleapis.com/auth/drive.file"
      fun token(context: Context, done: (TokenResult) -> Unit)            // sin interfaz
      fun startSignIn(activity: Activity, requestCode: Int, done: (TokenResult) -> Unit)
      fun finishSignIn(activity: Activity, data: Intent?): TokenResult  // en onActivityResult
      fun clearToken(context: Context, token: String, done: () -> Unit)
  }
  ```
  `token` y `startSignIn` llaman `Identity.getAuthorizationClient(context).authorize(...)`
  con `AuthorizationRequest.builder().setRequestedScopes(listOf(Scope(SCOPE)))`;
  `startSignIn`, si `hasResolution()`, lanza
  `activity.startIntentSenderForResult(pendingIntent.intentSender, requestCode, null, 0, 0, 0)`
  y no llama `done` hasta `finishSignIn`. `finishSignIn` usa
  `getAuthorizationResultFromIntent(data)`. Los callbacks llegan en el hilo principal.
  El token nunca se loguea.

- [ ] **Step 1:** escribir `DriveAuth.kt`.
- [ ] **Step 2:** `android/scripts/build-apk.sh aarch64` → `BUILD SUCCESSFUL`.
- [ ] **Step 3: commit** `feat: token de Drive con la Authorization API de Google`

### Tarea 5: `SyncManager`, cuándo y cómo sincronizar

**Files:**
- Create: `android/viewer/app/src/main/kotlin/io/github/yx14ng/rnote/viewer/SyncManager.kt` (~220 líneas)

**Interfaces:**
- Consumes: `DriveAuth` (tarea 4), `NativeBridge.syncRun`/`syncRevoke` (tarea 2), `BuildConfig.DRIVE_SYNC`.
- Produces:
  ```kotlin
  internal data class SyncReport(val uploaded: Int, val downloaded: Int, val moved: Int, val deleted: Int,
      val conflicts: List<String>, val changedHere: List<String>, val skipped: List<String>)
  internal sealed interface SyncStatus {
      data object Unavailable : SyncStatus   // build sin clave estable
      data object SignedOut : SyncStatus
      data object SignInNeeded : SyncStatus  // tenía sesión, Google pide al usuario de nuevo
      data object Running : SyncStatus
      data class Done(val atMs: Long, val report: SyncReport) : SyncStatus
      data class Pending(val message: String) : SyncStatus   // sin conexión
      data class Failed(val message: String) : SyncStatus
  }
  internal object SyncManager {
      val status: SyncStatus
      fun addListener(listener: (SyncStatus) -> Unit)     // recibe el estado actual al agregarse
      fun removeListener(listener: (SyncStatus) -> Unit)
      fun syncNow(context: Context)                       // no-op si Unavailable o SignedOut
      fun syncSoon(context: Context)                      // syncNow 5 s después del último llamado
      fun signedIn(context: Context, token: String)       // tras el inicio de sesión del botón
      fun signOut(context: Context, done: (Result<Unit>) -> Unit)
  }
  ```
  Comportamiento que el código debe cumplir (no lo cubren tests: se prueba en la tarea 7):
  - Preferencias `sync`: `enabled` (bool), `last_done_ms` (long). `status` inicial:
    `Unavailable` si `!BuildConfig.DRIVE_SYNC`, `SignedOut` si `!enabled`, si no `Done`
    con `last_done_ms` y un `SyncReport` vacío, o `Pending("")` si nunca terminó una.
  - Una sola sincronización a la vez en un `Executors.newSingleThreadExecutor()` propio;
    un `syncNow` durante `Running` marca `again` y se corre una más al terminar.
  - `syncNow`: `DriveAuth.token(applicationContext)`; `Token` → `syncRun(filesDir/library,
    filesDir/sync/state.json, Build.MODEL, token)` en el hilo; `NeedsUser` →
    `SignInNeeded` (sin abrir nada); `NotRegistered` → `Unavailable`; `Failed` → `Failed`.
  - Respuesta `unauthorized` → `DriveAuth.clearToken(token)`, token nuevo y un solo
    reintento por corrida; si vuelve `unauthorized` o el token pide usuario → `SignInNeeded`.
  - `offline` → `Pending(message)`; `busy` → encadena como `again`; `failed` → `Failed`;
    `ok` → `Done(now, report)` y `last_done_ms`.
  - `signOut`: token sin interfaz, `syncRevoke` en el hilo (si falla por conexión, igual
    se cierra la sesión local), `clearToken`, `enabled = false`, `SignedOut`. El estado
    `state.json` se conserva (otra cuenta cambia la carpeta raíz y `rnote-sync` empieza de cero).
  - Los listeners se llaman en el hilo principal. Nada de esto muestra UI.

- [ ] **Step 1:** escribir `SyncManager.kt`.
- [ ] **Step 2:** `android/scripts/build-apk.sh aarch64` → `BUILD SUCCESSFUL`.
- [ ] **Step 3: commit** `feat: SyncManager decide cuándo sincronizar y con qué token`

### Tarea 6: botón de nube en la biblioteca y el editor protegido

**Files:**
- Create: `android/viewer/app/src/main/kotlin/io/github/yx14ng/rnote/viewer/SyncButton.kt` (~180 líneas),
  `android/viewer/app/src/main/res/drawable/ic_cloud.xml`, `ic_cloud_off.xml`, `ic_cloud_sync.xml`
  (Material Symbols, Apache 2.0, mismo formato que `ic_sort.xml`)
- Modify: `LibraryActivity.kt`, `MainActivity.kt`, `DocumentSaver.kt`,
  `res/values/strings.xml`, `res/values-es/strings.xml`

**Interfaces:**
- Consumes: `SyncManager`, `DriveAuth`, `NativeBridge.libraryWriteNotebook`.
- Produces: `internal class SyncButton(activity: Activity) : ImageButton` con
  `fun attach()` / `fun detach()` (listener de `SyncManager`), `fun onActivityResult(requestCode, resultCode, data): Boolean`
  y `var onLibraryChanged: () -> Unit`. `DocumentSaver` gana
  `var libraryPath: String?`, `var knownSha256: String?` y
  `var onConflictCopy: (newPath: String) -> Unit`.

Comportamiento:
- **Botón** (en `LibraryActivity.updateBar`, antes de "Importar"; oculto mientras se selecciona):
  ícono `ic_cloud_off` para `Unavailable`/`SignedOut`/`SignInNeeded`, `ic_cloud_sync` para
  `Running`, `ic_cloud` para `Done`/`Pending`/`Failed` (con alpha 0.6 en `Pending` y `Failed`).
  Toque:
  - `Unavailable` → diálogo `sync_unavailable` ("Esta compilación no puede sincronizar con
    Google Drive: no está firmada con la clave registrada en Google.").
  - `SignedOut`/`SignInNeeded` → `DriveAuth.startSignIn(activity, REQUEST_SIGN_IN)`; con el
    token, `SyncManager.signedIn(...)`; `NotRegistered` → el mismo diálogo; `Failed` → toast.
  - Con sesión → `PopupMenu`: primera línea deshabilitada con el estado
    (`sync_status_done` "Sincronizado %s" con `DateUtils.getRelativeTimeSpanString`,
    `sync_status_pending` "Pendiente: %s", `sync_status_running`, `sync_status_failed`),
    "Sincronizar ahora", "Cerrar sesión de Google Drive" (confirma con un diálogo).
- **Biblioteca:** `onResume` → `syncButton.attach()` y `SyncManager.syncNow(this)`;
  `onPause` → `detach()`. Al pasar a `Done` con `changedHere` o `conflicts` no vacíos →
  `reload()`; con `conflicts` no vacíos, toast `sync_conflicts` (plural: "Se guardaron %d
  copias de conflicto"). `Failed` → un toast por cambio de estado, no por intento.
- **Editor:** `DocumentSaver.writeTo` de un cuaderno de la biblioteca (`libraryPath != null`)
  escribe con `NativeBridge.libraryWriteNotebook(root, libraryPath, knownSha256, Build.MODEL, bytes)`
  en vez de `DocumentFiles.write`; guarda el `sha256` devuelto en `knownSha256`; si
  `conflict`, `onConflictCopy(path)`: `MainActivity` pasa `libraryPath`,
  `saver.currentUri` y `saver.documentName` al cuaderno nuevo y muestra el toast
  `sync_saved_as_copy` ("Este cuaderno cambió en otro dispositivo: tus cambios se
  guardaron como una copia"). `openLibraryNotebook` calcula `knownSha256` de los bytes
  que lee (`MessageDigest.getInstance("SHA-256")`, hex en minúscula como Rust); un cuaderno
  nuevo empieza con `null`. Después de cada guardado exitoso de un cuaderno de la
  biblioteca, `SyncManager.syncSoon(this)`. Los archivos abiertos por fuera de la
  biblioteca (`libraryPath == null`) se guardan como hasta ahora.
- **Recarga en el editor:** `MainActivity` agrega un listener en `onResume` y lo saca en
  `onPause`. Si un `Done` trae `libraryPath` en `changedHere`: sin cambios sin guardar → si
  el archivo existe, se reabre con `openLibraryNotebook(path, isNew = false)` y toast
  `sync_reloaded`; si no existe (se borró en otro dispositivo), toast `sync_removed` y vuelve
  a la biblioteca; con cambios sin guardar → nada (el guardado protegido decide).

- [ ] **Step 1:** íconos y textos (en ambos `strings.xml`).
- [ ] **Step 2:** `SyncButton.kt` y el cableado en `LibraryActivity.kt`
  (`onActivityResult` delega primero en `syncButton.onActivityResult`).
- [ ] **Step 3:** guardado protegido en `DocumentSaver.kt` y el cableado en `MainActivity.kt`.
- [ ] **Step 4:** `android/scripts/build-apk.sh aarch64` → `BUILD SUCCESSFUL`;
  `cd android/viewer && ./gradlew --no-daemon lintDebug` sin avisos nuevos: la cantidad de
  avisos del informe no supera el conteo base de la tarea 3 (los nuevos de Play Services
  que no son del código de la app se anotan como ruling).
- [ ] **Step 5: commits** (uno por tipo): `feat: botón de Google Drive en la biblioteca y sincronización automática`
  y `feat: el editor no pisa un cuaderno que cambió en otro dispositivo`.

### Tarea 7: documentación y prueba en la tablet

**Files:**
- Modify: `android/README.md` (sección "Google Drive sync": cómo se activa, que necesita la
  clave estable, dónde vive el estado, qué hace "Cerrar sesión")

- [ ] **Step 1:** README; commit `docs: sincronización con Google Drive en Android`.
- [ ] **Step 2:** instalar el APK por adb (respetando la app en primer plano) y, con el
  usuario mirando, recorrer la lista; anotar el resultado de cada punto en el ledger:
  1. Iniciar sesión desde el botón: aparece el selector de cuentas de Google, después
     "Sincronizado hace un momento"; en drive.google.com aparece la carpeta `Rnote` con
     los cuadernos y `.rnote-library/library.json`.
  2. Crear un cuaderno, escribir, volver: unos segundos después está en Drive.
  3. Borrar en Drive (web) un archivo que la app subió; volver a la biblioteca: el
     cuaderno desaparece de la tablet y queda en `.rnote-library/.sync-removed/`.
  4. Modo avión, volver a la biblioteca: el botón queda "Pendiente", sin toasts repetidos;
     quitar el modo avión y volver: sincroniza.
  5. Cuaderno abierto con cambios sin guardar mientras el archivo cambia por debajo (con
     `drive.file`, una subida desde la web es otro archivo que la app no ve, así que se
     simula lo que haría una bajada: `adb shell run-as io.github.yx14ng.rnote.viewer cp`
     de otro cuaderno encima del abierto): al guardar, aparece la copia "(conflicto, …)"
     con los cambios del editor y el archivo de abajo queda intacto.
  6. "Cerrar sesión de Google Drive": el botón vuelve a "Iniciar sesión"; en
     myaccount.google.com/permissions Rnote ya no figura.
- [ ] **Step 3:** push de la rama al cerrar la etapa.
