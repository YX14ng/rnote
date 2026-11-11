# Etapa 3: `rnote-sync` contra un Drive falso — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un crate `rnote-sync` que sincroniza la carpeta de una biblioteca con una carpeta
`Rnote` de Google Drive sin perder nunca una versión, probado de punta a punta contra un
Drive falso en memoria (uno, dos y tres dispositivos, cortes a mitad de camino), más el
cliente REST de Drive v3 probado contra un servidor HTTP falso local. Sin interfaz, sin
OAuth y sin proyecto de Google Cloud: eso es la etapa 4.

**Architecture:** `rnote-library` gana la mezcla de `library.json` por entrada. `rnote-sync`
define un trait `Remote` (las ocho operaciones de Drive que usa el algoritmo), con dos
implementaciones: `FakeDrive` (memoria, compartible entre "dispositivos" de un test) y
`DriveClient` (REST con `ureq`). `sync()` toma el candado, lee el estado local, lista lo
remoto, escanea lo local, aplica movimientos, compara contenido en tres vías y guarda el
estado después de cada archivo.

**Tech Stack:** Rust 2024 del workspace (rust-version 1.89), `serde`/`serde_json`,
`anyhow`, `thiserror`, `chrono`, `sha2`, `md-5` (solo el Drive falso), `ureq` 3 con
rustls y el proveedor `ring`, `tempfile` en tests.

**Spec:** `docs/superpowers/specs/2026-11-10-biblioteca-sync-busqueda-design.md` (secciones
"Arquitectura", "Biblioteca / En disco", "Sincronización con Google Drive": "En Drive",
"Algoritmo", "Distribución pública" (backoff); "Verificación"; "Orden propuesto" etapa 3).

## Global Constraints

- `rnote-sync` no depende de GTK, de Android ni de `rnote-engine` directamente (sí de
  `rnote-library`, para la mezcla y las constantes de rutas).
- Código nuevo: Google Rust Style Guide; `crates/rnote-sync/rustfmt.toml` igual al de
  `crates/rnote-library/rustfmt.toml`; mismo bloque `[lints]` que `rnote-library`
  (incluye `missing_docs` y `unwrap_used`; los tests llevan
  `#![allow(clippy::unwrap_used)]` como en `rnote-library/src/tests/mod.rs`).
- Tests como módulos en `src/tests/` (igual que `rnote-library`), no en `tests/`.
- Archivos de 200–400 líneas típico, 800 tope.
- En Drive nunca se borra en forma permanente: solo `trashed = true`. Ningún request
  `DELETE` en `drive.rs`.
- Scope único `https://www.googleapis.com/auth/drive.file`; el token lo da quien llama.
  Ninguna credencial (client id, secret, token) en el repo ni en los tests.
- HTTP con `ureq` 3 y rustls con proveedor `ring`: `cargo tree -p rnote-sync -i aws-lc-rs`
  no debe encontrar el paquete (aws-lc-rs complica el build de Android).
- Nombre de la copia de conflicto, literal del spec:
  `<carpeta>/<Nombre> (conflicto, <dispositivo>, <fecha>).rnote`, con
  `<fecha>` = `%Y-%m-%d %H.%M` (sin `:`, que exFAT no admite). Si existe, sufijo ` (2)`, ` (3)`…
- Se sincroniza solo: archivos `.rnote` (extensión sin distinguir mayúsculas) en la raíz y
  en subcarpetas no ocultas; `.rnote-library/library.json`; `.rnote-library/trash/*.rnote`.
  Todo lo demás (ocultos, `.xopp`, `library.json.tmp`, `library.json.bad`, temporales)
  se ignora.
- El estado de sincronización vive fuera de la biblioteca, en la ruta que pasa quien llama.
- Commits en español `tipo: descripción`, uno por tipo. Verificación antes de cada commit:
  `cargo test -p rnote-library -p rnote-sync`,
  `cargo clippy -p rnote-library -p rnote-sync --all-targets -- -D warnings`,
  `cargo fmt -p rnote-library -p rnote-sync -- --check`. Push al cerrar la etapa.
  (`CARGO_HOME=$PWD/_mesonbuild/cargo-home` como en la etapa 2.)

## Review Focus

1. La carpeta `Rnote` de Drive fue borrada o reemplazada (el `root_id` del estado ya no
   está): no se borra ningún cuaderno local; se vuelve a subir todo. Test en la tarea 4.
2. Primera sincronización de un segundo dispositivo que ya tiene cuadernos con los mismos
   nombres y otro contenido: copias de conflicto, nada pisado. Test en la tarea 5.
3. Dos archivos remotos con el mismo nombre en la misma carpeta (dos dispositivos crearon
   "Untitled.rnote" sin conexión): los dos terminan en todos los dispositivos. Test en la
   tarea 5.
4. Corte entre la subida de un archivo nuevo y el guardado del estado: la sincronización
   siguiente no crea un duplicado en Drive. Test en la tarea 7.
5. Archivos que no son de la biblioteca (`.Orga.autosave.xopp`, `.xopp`,
   `library.json.tmp`, `library.json.bad`, temporales de la propia sync) nunca se suben.
   Test en la tarea 3.

---

## Estructura de archivos

```
crates/rnote-library/src/meta.rs          + merge_meta (pub), META_FILE pub
crates/rnote-library/src/tests/meta.rs    + tests de la mezcla
crates/rnote-sync/
  Cargo.toml, rustfmt.toml
  src/lib.rs        SyncContext, SyncReport, SyncError, pub fn sync, re-exports
  src/remote.rs     trait Remote, RemoteFile, RemoteFolder, Listing, RemoteError
  src/fake.rs       FakeDrive (cfg(any(test, feature = "fake")))
  src/local.rs      is_synced, scan, sha256_hex, write_atomic
  src/state.rs      SyncState, Entry
  src/names.rs      conflict_name, free_path
  src/lock.rs       SyncLock
  src/tree.rs       RemoteTree: rutas ↔ ids de carpetas bajo la raíz, crear y limpiar carpetas
  src/moves.rs      movimientos remotos (por id) y locales (por hash)
  src/content.rs    comparación de contenido en tres vías y su ejecución
  src/drive.rs      DriveClient (ureq)
  src/tests/        mod.rs (Device, helpers), fake.rs, local.rs, basic.rs, conflicts.rs,
                    moves.rs, devices.rs, stub_http.rs, drive.rs
```

`Cargo.toml` del workspace: miembro `crates/rnote-sync`, dependencia
`rnote-sync = { version = "0.15.0", path = "crates/rnote-sync" }`, y en
`[workspace.dependencies]` `sha2`, `md-5`, `ureq` (última 3.x, `default-features = false`,
features de rustls con `ring`; elegir los nombres exactos de features de la versión
publicada).

---

### Task 1: mezcla de `library.json`

**Files:** Modify `crates/rnote-library/src/meta.rs`, `crates/rnote-library/src/lib.rs`;
Test `crates/rnote-library/src/tests/meta.rs`.

**Interfaces:**
- Produces: `pub fn merge_meta(local: &[u8], remote: &[u8]) -> anyhow::Result<Vec<u8>>`
  y `pub const META_FILE: &str = "library.json"`, re-exportados en `lib.rs`.

- [ ] **Step 1: tests que fallan** (en `tests/meta.rs`, armando `Meta` como JSON a mano):
  - `merge_keeps_entries_from_both_sides`: tag `a` solo local, tag `b` solo remoto →
    la salida tiene `a` y `b`.
  - `merge_takes_the_newer_record_of_each_entry`: tag `a` local `updated_at: 5` nombre
    "Viejo", remoto `updated_at: 9` "Nuevo" → "Nuevo"; y al revés por entrada distinta en
    `notebook_tags` → gana la local.
  - `merge_keeps_tombstones`: `trash` entrada `x` remota `deleted: true, updated_at: 9`,
    local viva `updated_at: 5` → sale `deleted: true`.
  - `merge_is_commutative`: con un empate de `updated_at` (uno borrado y otro vivo),
    `merge_meta(a, b) == merge_meta(b, a)` byte a byte.
  - `merge_with_an_unreadable_side_keeps_the_readable_one`: `remote = b"{"` → la salida
    se parsea igual que `local`; y al revés.
  - `merge_of_two_unreadable_sides_is_an_error`.
- [ ] **Step 2:** `cargo test -p rnote-library merge_` → FAIL (no existe `merge_meta`).
- [ ] **Step 3: implementar.** Por cada mapa (`tags`, `notebook_tags`, `trash`): unión de
  claves; si está en los dos lados gana el mayor `updated_at`; empate → gana el registro
  con `deleted == true`, y si siguen empatados el de mayor `serde_json::to_string`
  (determinista, para que todos los dispositivos converjan). Un lado ilegible o de otra
  `version` se trata como ausente. Salida: `serde_json::to_vec_pretty`, igual que `save`.
- [ ] **Step 4:** `cargo test -p rnote-library` → todo PASS.
- [ ] **Step 5: commit** `feat: mezclar library.json entrada por entrada`.

### Task 2: crate `rnote-sync`, el trait `Remote` y el Drive falso

**Files:** Create `crates/rnote-sync/{Cargo.toml,rustfmt.toml,src/lib.rs,src/remote.rs,src/fake.rs,src/tests/mod.rs,src/tests/fake.rs}`; Modify `Cargo.toml` (workspace).

**Interfaces:**
- Produces (en `remote.rs`, re-exportados):
  ```rust
  pub const ROOT_FOLDER_NAME: &str = "Rnote";
  pub struct RemoteFile { pub id: String, pub parent: String, pub name: String,
                          pub md5: String, pub modified: String }
  pub struct RemoteFolder { pub id: String, pub parent: Option<String>, pub name: String }
  pub struct Listing { pub files: Vec<RemoteFile>, pub folders: Vec<RemoteFolder> }
  #[derive(Debug, thiserror::Error)]
  pub enum RemoteError { Unauthorized, Offline(String), Failed(anyhow::Error) }
  pub trait Remote {
      /// The folder named `Rnote` at the top of My Drive, if the app can see one.
      fn find_root(&mut self) -> Result<Option<String>, RemoteError>;
      /// Every file and folder the app can see that isn't trashed.
      fn list(&mut self) -> Result<Listing, RemoteError>;
      fn download(&mut self, id: &str) -> Result<Vec<u8>, RemoteError>;
      fn create_file(&mut self, parent: &str, name: &str, bytes: &[u8])
          -> Result<RemoteFile, RemoteError>;
      fn update_file(&mut self, id: &str, bytes: &[u8]) -> Result<RemoteFile, RemoteError>;
      fn move_file(&mut self, id: &str, from_parent: &str, to_parent: &str, name: &str)
          -> Result<RemoteFile, RemoteError>;
      /// `parent: None` = the top of My Drive.
      fn create_folder(&mut self, parent: Option<&str>, name: &str)
          -> Result<String, RemoteError>;
      fn trash(&mut self, id: &str) -> Result<(), RemoteError>;
  }
  ```
- Produces (en `fake.rs`, `#[cfg(any(test, feature = "fake"))] pub mod fake;`, feature
  `fake = []`): `FakeDrive` con `Clone` que comparte el mismo Drive (`Rc<RefCell<…>>`):
  `new()`, `fail_after(&self, ops: usize)` (desde la operación número `ops + 1`, toda
  operación devuelve `RemoteError::Offline`; `fail_never()` lo apaga), `ops(&self) -> usize`,
  `paths(&self) -> Vec<String>` (archivos vivos bajo `Rnote`, rutas relativas, ordenadas),
  `content(&self, path: &str) -> Option<Vec<u8>>`, `trashed_paths(&self) -> Vec<String>`,
  `add_file(&self, path: &str, bytes: &[u8])` (como otro cliente; crea carpetas y la raíz si
  faltan), `trash_root(&self)`.
  `md5` = hex de MD5 real del contenido; `modified` = contador creciente con formato
  RFC 3339. Como Drive, admite dos archivos con el mismo nombre en una carpeta. Trashear
  una carpeta trashea su contenido.

- [ ] **Step 1: tests que fallan** en `tests/fake.rs`:
  `create_and_list_builds_paths_under_the_root`, `same_name_twice_keeps_both_files`,
  `trash_hides_a_file_from_the_listing_but_keeps_it`, `trashing_a_folder_hides_its_files`,
  `fail_after_makes_later_operations_offline`, `md5_is_the_md5_of_the_content`
  (MD5 de `b"abc"` = `900150983cd24fb0d6963f7d28e17f72`).
- [ ] **Step 2:** `cargo test -p rnote-sync` → FAIL (no compila: tipos inexistentes).
- [ ] **Step 3:** crate con `lib.rs` (doc del crate, módulos, re-exports), `remote.rs`,
  `fake.rs`.
- [ ] **Step 4:** `cargo test -p rnote-sync` → PASS; clippy y fmt limpios.
- [ ] **Step 5: commit** `feat: crate rnote-sync con el trait Remote y un Drive falso`.

### Task 3: escaneo local, estado, nombres y candado

**Files:** Create `src/local.rs`, `src/state.rs`, `src/names.rs`, `src/lock.rs`,
`src/tests/local.rs`.

**Interfaces:**
- `local.rs`: `pub(crate) fn is_synced(path: &str) -> bool`;
  `pub(crate) fn scan(root: &Path) -> anyhow::Result<BTreeMap<String, String>>`
  (ruta relativa con `/` → SHA-256 hex; no sigue symlinks);
  `pub(crate) fn sha256_hex(bytes: &[u8]) -> String`;
  `pub(crate) fn write_atomic(root: &Path, path: &str, bytes: &[u8]) -> anyhow::Result<()>`
  (crea carpetas; escribe `.<nombre>.sync-tmp` en la misma carpeta y renombra).
- `state.rs`: `#[derive(Serialize, Deserialize, Default)] pub(crate) struct SyncState
  { pub root_id: Option<String>, pub entries: BTreeMap<String, Entry> }`,
  `pub(crate) struct Entry { pub id: String, pub md5: String, pub modified: String,
  pub sha256: String }`; `load(path) -> anyhow::Result<Self>` (inexistente o ilegible →
  `Default`, el ilegible se aparta como `<ruta>.bad`); `save(&self, path)` atómico.
- `names.rs`: `pub(crate) fn conflict_name(path: &str, device: &str, now: NaiveDateTime)
  -> String`; `pub(crate) fn free_path(wanted: &str, taken: &dyn Fn(&str) -> bool) -> String`
  (prueba `wanted`, después ` (2)`, ` (3)`… antes de la extensión). En `device`, `/`, `\`,
  `:` y caracteres de control pasan a `-`.
- `lock.rs`: `pub(crate) struct SyncLock(File)`;
  `pub(crate) fn acquire(state_path: &Path) -> anyhow::Result<Option<SyncLock>>` sobre
  `<state_path>.lock` con `File::try_lock` (`WouldBlock` → `None`).

- [ ] **Step 1: tests que fallan** en `tests/local.rs`:
  - `scan_takes_notebooks_library_json_and_trash` (raíz, `Orga/a.rnote`,
    `.rnote-library/library.json`, `.rnote-library/trash/1-a.rnote`).
  - `scan_skips_hidden_temporary_and_other_files` (Review Focus 5): `.Orga.autosave.xopp`,
    `Parciales.xopp`, `.rnote-library/library.json.tmp`, `.rnote-library/library.json.bad`,
    `.a.rnote.sync-tmp`, `.oculta/x.rnote`, `notas.txt` → ninguno aparece.
  - `scan_accepts_upper_case_extension` (`A.RNOTE`).
  - `conflict_name_has_device_and_date`: `conflict_name("Orga/Parcial.rnote",
    "Galaxy Tab S10", 2026-11-10 21:50)` == `"Orga/Parcial (conflicto, Galaxy Tab S10,
    2026-11-10 21.50).rnote"`; con `device = "a/b:c"` → `a-b-c`.
  - `free_path_adds_a_number_ignoring_case`: tomado `"orga/x.rnote"` → `free_path(
    "Orga/X.rnote", …)` == `"Orga/X (2).rnote"`.
  - `state_round_trips_and_unreadable_state_is_empty`.
  - `second_lock_is_refused_until_the_first_is_dropped`.
- [ ] **Step 2:** `cargo test -p rnote-sync` → FAIL.
- [ ] **Step 3:** implementar los cuatro módulos.
- [ ] **Step 4:** PASS + clippy + fmt.
- [ ] **Step 5: commit** `feat: escaneo local, estado, nombres de conflicto y candado de sincronización`.

### Task 4: `sync()` — subir, bajar, borrar, carpetas y raíz

**Files:** Create `src/tree.rs`, `src/content.rs`, `src/tests/basic.rs`; Modify `src/lib.rs`.

**Interfaces:**
- Produces (en `lib.rs`):
  ```rust
  pub struct SyncContext<'a> { pub library_root: &'a Path, pub state_path: &'a Path,
                               pub device: &'a str, pub now: chrono::NaiveDateTime }
  #[derive(Debug, Default, PartialEq)]
  pub struct SyncReport { pub uploaded: usize, pub downloaded: usize, pub moved: usize,
                          pub deleted: usize, pub conflicts: Vec<String> }
  #[derive(Debug, thiserror::Error)]
  pub enum SyncError { Busy, Unauthorized, Offline(String), Failed(anyhow::Error) }
  pub fn sync(ctx: &SyncContext, remote: &mut dyn Remote) -> Result<SyncReport, SyncError>;
  ```
  `conflicts` = rutas locales de las copias de conflicto creadas.
- `tree.rs`: `RemoteTree::new(listing: &Listing, root_id: &str)` → archivos vivos por ruta
  relativa (una ruta puede tener varios ids) y carpetas por ruta;
  `ensure_folder(&mut self, remote, path) -> Result<String, RemoteError>` crea las que
  faltan.
- `src/tests/mod.rs`: `Device { dir: TempDir, state: PathBuf, name: &'static str }` con
  `sync(&self, drive: &FakeDrive) -> SyncReport` (fecha fija 2026-11-10 21:50),
  `write`, `read`, `delete`, `paths`.

**Orden de `sync()`:** candado (`None` → `Busy`); `SyncState::load`; `list()`; raíz =
`state.root_id` si está entre las carpetas listadas, si no `find_root()`, si no
`create_folder(None, ROOT_FOLDER_NAME)`; **si la raíz cambió respecto de `state.root_id`,
se vacía `state.entries`** (Review Focus 1) y se guarda; `scan()`; movimientos (tarea 6);
contenido; limpieza de carpetas. Después de cada operación sobre un archivo se actualiza
la entrada y se hace `state.save`. `RemoteError::Unauthorized` → `SyncError::Unauthorized`,
`Offline` → `Offline`.

**Reglas de contenido de esta tarea** (L = hay archivo local en la ruta; S = hay entrada de
estado; R = en filas con S, el id de la entrada sigue vivo en esa misma ruta; en filas sin
S, hay algún archivo remoto en la ruta. "Cambió local" = sha256 ≠ `S.sha256`; "cambió
remoto" = md5 ≠ `S.md5`. Hasta la tarea 6, un movimiento remoto se ve como borrado + alta):

| S | L | R | Acción |
|---|---|---|---|
| sí | sí | sí | solo local cambió → `update_file`; solo remoto → bajar; ninguno → nada (los dos: tarea 5) |
| sí | sí | no | local cambió → subir como nuevo (gana el modificado); si no → borrar el archivo local |
| sí | no | sí | remoto cambió → bajar; si no → `trash` remoto |
| sí | no | no | quitar la entrada |
| no | sí | no | subir nuevo (crea carpetas) |
| no | no | sí | bajar |
| no | sí | sí | bajar; mismo sha256 → solo registrar (distinto: tarea 5) |

Limpieza: se trashea toda carpeta remota bajo la raíz (salvo `.rnote-library` y
`.rnote-library/trash`) sin archivos vivos debajo cuya ruta no existe como carpeta local;
se borra toda carpeta local no oculta que quedó vacía porque esta sincronización le sacó
un archivo y que no existe en remoto.

- [ ] **Step 1: tests que fallan** en `tests/basic.rs` (dos `Device` y un `FakeDrive`):
  `new_local_notebook_is_uploaded_into_its_folder`, `new_remote_notebook_is_downloaded`,
  `local_edit_is_uploaded`, `remote_edit_is_downloaded`,
  `nothing_changed_does_no_operation` (en una segunda sincronización sin cambios,
  `drive.ops()` sube exactamente 1: el `list`),
  `local_delete_trashes_the_remote_file` (queda en `trashed_paths`),
  `remote_delete_removes_the_unchanged_local_file`,
  `remote_delete_of_a_locally_edited_file_restores_it`,
  `local_delete_of_a_remotely_edited_file_downloads_it`,
  `empty_remote_folder_is_trashed_and_local_folder_removed`,
  `deleted_root_folder_never_deletes_local_notebooks` (Review Focus 1:
  `drive.trash_root()`, sincronizar → los archivos locales siguen y están todos en la
  carpeta raíz nueva), `skipped_files_are_never_uploaded` (Review Focus 5, por la ruta de
  `sync`), `busy_lock_returns_busy`.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** implementar `tree.rs`, `content.rs` y `sync()`.
- [ ] **Step 4:** PASS + clippy + fmt.
- [ ] **Step 5: commit** `feat: sincronización en tres vías de cuadernos y carpetas`.

### Task 5: conflictos, `library.json`, duplicados y mayúsculas

**Files:** Modify `src/content.rs`; Test `src/tests/conflicts.rs`.

**Reglas:**
- Los dos cambiaron (o nuevo en los dos con distinto contenido): bajar lo remoto; mismo
  sha256 → registrar; si no, escribir lo remoto en `conflict_name(...)` (libre según
  `free_path`), subirlo como archivo nuevo, y `update_file` del original con lo local. La
  ruta de la copia va a `report.conflicts`.
- `.rnote-library/library.json` nunca es conflicto: si cambiaron los dos (o es nuevo en
  los dos), `merge_meta(local, remoto)`, escribir local con `write_atomic` y `update_file`.
- Varios ids en una ruta remota: se queda el de `S` si está; si no, el de `modified` más
  viejo (empate: menor id). Cada otro se baja a `conflict_name` y se renombra en remoto a
  ese nombre con `move_file` (Review Focus 3).
- Bajar una ruta remota nueva cuya versión en minúsculas coincide con otra ruta local
  distinta: bajarla a `free_path` (comparando sin mayúsculas) y renombrar el archivo
  remoto a esa ruta.

- [ ] **Step 1: tests que fallan:** `both_edited_keeps_both_versions_everywhere` (después
  de sincronizar los dos dispositivos dos veces, los dos tienen original + copia, con el
  contenido de cada uno), `same_name_created_offline_on_two_devices_keeps_both`
  (Review Focus 2), `two_remote_files_with_one_name_end_up_on_every_device`
  (Review Focus 3, con `add_file` dos veces), `library_json_edited_on_both_is_merged`
  (las etiquetas de los dos lados quedan en los dos dispositivos y en Drive),
  `names_differing_only_in_case_do_not_overwrite` (`Orga/a.rnote` local y `Orga/A.rnote`
  remoto con otro contenido → las dos versiones quedan, ninguna pisada),
  `conflict_copy_name_is_free` (si la copia ya existe, ` (2)`).
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS + clippy + fmt.
- [ ] **Step 5: commit** `feat: copias de conflicto y mezcla de library.json al sincronizar`.

### Task 6: movimientos y renombres

**Files:** Create `src/moves.rs`; Modify `src/lib.rs`; Test `src/tests/moves.rs`.

**Reglas (antes de la fase de contenido):**
1. Remotos: entrada de `S` en `p` cuyo id aparece en otra ruta `q` → mover el archivo
   local `p` → `q` si existe (si `q` está tomado localmente, `free_path` y `move_file`
   remoto a ese nombre), y re-clavear la entrada a `q`; el contenido se resuelve después
   con las reglas normales.
2. Locales: entrada de `S` en `p` sin archivo local, con su id remoto en `p` y sin cambios
   remotos; si hay exactamente una ruta local `q` que no está en `S` ni en remoto con el
   mismo sha256 → `move_file` (creando carpetas) y re-clavear. Varias candidatas → la
   primera por orden de ruta; las demás siguen como nuevas.
- Mandar a la papelera desde la app es un movimiento local hacia
  `.rnote-library/trash/<id>-<nombre>.rnote`; restaurar, el movimiento inverso.

- [ ] **Step 1: tests que fallan:** `local_rename_moves_the_remote_file_without_upload`
  (el id remoto no cambia y no hubo `create_file`), `remote_move_moves_the_local_file`,
  `trash_in_the_app_reaches_the_other_device` (con `rnote_library::Library::trash` en un
  dispositivo: en el otro el archivo queda en la papelera de la biblioteca y la entrada de
  papelera de `library.json` llega mezclada), `moved_and_edited_on_one_device_arrives_moved_and_edited`,
  `remote_move_onto_a_taken_local_name_keeps_both`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar `moves.rs`. **Step 4:** PASS + clippy + fmt.
- [ ] **Step 5: commit** `feat: movimientos y renombres sin volver a subir`.

### Task 7: cortes a mitad de camino y tres dispositivos

**Files:** Test `src/tests/devices.rs`; Modify lo que los tests muestren roto.

- [ ] **Step 1: tests** (pueden pasar de entrada; si pasan, igual quedan como red):
  - `interrupted_sync_finishes_without_duplicates` (Review Focus 4): escenario con altas,
    ediciones, un movimiento y un borrado; para cada `n` desde 0 hasta las operaciones de
    una corrida limpia: Drive y dispositivo nuevos, `fail_after(n)`, `sync` (falla con
    `Offline`), `fail_never()`, `sync` → `drive.paths()`, sus contenidos y los archivos
    locales iguales a los de la corrida limpia, sin rutas repetidas en Drive.
  - `three_devices_converge`: A, B y C editan cuadernos distintos y uno en común sin
    sincronizar; sincronizar A, B, C, A, B → los tres directorios iguales a Drive, con una
    copia de conflicto por cada edición concurrente del cuaderno común.
  - `offline_returns_offline_and_changes_nothing_locally` (`fail_after(0)`).
- [ ] **Step 2:** correr; documentar en el ledger qué falló y por qué.
- [ ] **Step 3:** corregir con TDD lo que falle (el test que falla es el de este paso).
- [ ] **Step 4:** suite completa PASS + clippy + fmt.
- [ ] **Step 5: commit** `test: cortes a mitad de camino y tres dispositivos` (y `fix:` aparte si hubo correcciones).

### Task 8: `DriveClient` REST

**Files:** Create `src/drive.rs`, `src/tests/stub_http.rs`, `src/tests/drive.rs`; Modify
`Cargo.toml` de `rnote-sync` y del workspace.

**Interfaces:**
- Produces: `pub struct DriveClient`, `impl Remote for DriveClient`;
  `DriveClient::new(token: impl FnMut() -> Result<String, RemoteError> + 'static) -> Self`,
  `.with_base_url(url: &str) -> Self` (por defecto `https://www.googleapis.com`),
  `.with_sleep(f: impl FnMut(Duration) + 'static) -> Self` (por defecto `thread::sleep`).
- `stub_http.rs`: `Stub::start(responses: Vec<(u16, &str /*content-type*/, Vec<u8>)>)
  -> Stub` (TcpListener en `127.0.0.1:0`, un hilo, responde en orden con
  `Connection: close`), `stub.url()`, `stub.requests() -> Vec<Request { method, target,
  headers, body }>`.

**Requests** (todas con `Authorization: Bearer <token>`; `q` y demás parámetros
codificados por `ureq`):
- `find_root`: `GET /drive/v3/files?q=name = 'Rnote' and mimeType = 'application/vnd.google-apps.folder' and 'root' in parents and trashed = false&fields=files(id)&spaces=drive`.
- `list`: `GET /drive/v3/files?q=trashed = false&fields=nextPageToken,files(id,name,mimeType,parents,md5Checksum,modifiedTime)&pageSize=1000&spaces=drive`, siguiendo `nextPageToken`. Carpeta = `mimeType` de carpeta; los demás son archivos.
- `download`: `GET /drive/v3/files/{id}?alt=media`.
- `create_file`: `POST /upload/drive/v3/files?uploadType=multipart&fields=<campos de archivo>`, `multipart/related` con una parte JSON `{"name","parents":[parent]}` y una `application/octet-stream`.
- `update_file`: `PATCH /upload/drive/v3/files/{id}?uploadType=media&fields=…`, cuerpo = bytes.
- `move_file`: `PATCH /drive/v3/files/{id}?addParents=…&removeParents=…&fields=…`, JSON `{"name"}`.
- `create_folder`: `POST /drive/v3/files?fields=id`, JSON `{"name","mimeType":"application/vnd.google-apps.folder"}` más `"parents"` si hay padre.
- `trash`: `PATCH /drive/v3/files/{id}?fields=id`, JSON `{"trashed":true}`.

Errores: 401 → `Unauthorized`; 429, 5xx y 403 con `reason` `rateLimitExceeded` o
`userRateLimitExceeded` → reintento con esperas de 1, 2, 4, 8 y 16 s y después `Failed`;
error de conexión, DNS o timeout → `Offline`; cualquier otro → `Failed` con estado y
cuerpo.

- [ ] **Step 1: tests que fallan** en `tests/drive.rs`:
  `list_follows_page_tokens_and_splits_files_and_folders`,
  `create_file_sends_multipart_with_name_and_parent`, `update_file_sends_the_bytes`,
  `move_file_sends_add_and_remove_parents_and_name`, `trash_never_deletes` (método
  `PATCH`, cuerpo `{"trashed":true}`), `every_request_carries_the_bearer_token`,
  `unauthorized_is_401`, `retries_429_then_succeeds` (esperas registradas == `[1s, 2s]`),
  `rate_limit_403_is_retried_but_other_403_is_not`, `refused_connection_is_offline`.
- [ ] **Step 2:** FAIL. **Step 3:** implementar. **Step 4:** PASS + clippy + fmt;
  `cargo tree -p rnote-sync -i aws-lc-rs` sin resultados; `grep -n 'DELETE\|\.delete(' crates/rnote-sync/src/drive.rs` vacío.
- [ ] **Step 5:** compilación cruzada: `. android/scripts/env.sh aarch64 && cargo clippy -p rnote-sync --target aarch64-linux-android -- -D warnings` → sin avisos (si el entorno de Android no está, anotarlo en el ledger como no verificado).
- [ ] **Step 6: commit** `feat: cliente REST de Google Drive para rnote-sync`. Push al cerrar la etapa.

---

## Lo que queda para las etapas 4 y 5 (no se hace acá)

OAuth (Android y escritorio), `keyring`, botones y disparadores de sincronización,
recargar la `Library` y los cuadernos abiertos después de bajar archivos, nombre del
dispositivo (`Build.MODEL` / hostname), carpetas que solo cambian mayúsculas entre
dispositivos (renombre en dos pasos en exFAT), carpetas vacías (no se sincronizan: el
spec dice que las carpetas siguen a los archivos).
