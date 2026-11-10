# Etapa 1: biblioteca y barra estilo StarNote en Android — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** La app Android abre en una biblioteca de cuadernos (carpetas, etiquetas, papelera,
miniaturas, importar) y el editor usa una barra superior de dos filas con `<` para volver,
firmada con un keystore estable.

**Architecture:** Un crate nuevo `crates/rnote-library` (Rust, sin GTK ni Android) maneja la
biblioteca en disco y su `library.json`; `rnote-android` lo expone por JNI con un único punto
de entrada JSON más uno binario para miniaturas. En Kotlin, una `LibraryActivity` nueva es el
lanzador y abre `MainActivity` con el archivo; el editor reemplaza la ActionBar y la barra
lateral por `EditorTopBar`.

**Tech Stack:** Rust 2024 (workspace de Rnote, `rnote-engine`, `serde`/`serde_json`), JNI
(`jni` 0.22), Kotlin con widgets del framework (sin AndroidX), Gradle/AGP del proyecto.

**Spec:** `docs/superpowers/specs/2026-11-10-biblioteca-sync-busqueda-design.md` (secciones
"Biblioteca", "Editor Android: barra estilo StarNote", "Proyecto de Google Cloud" → keystore,
"Orden propuesto" etapa 1).

## Global Constraints

- Código Rust nuevo con Google Rust Style Guide; `rnote-library` copia la sección
  `[lints]` de `crates/rnote-android/Cargo.toml`. No reformatear crates upstream.
- Archivos de 200–400 líneas típico, 800 tope.
- Android: `minSdk = 28`, sin AndroidX ni librerías nuevas en esta etapa; textos en
  `res/values/strings.xml` (EN) y `res/values-es/strings.xml` (ES).
- Ninguna credencial en el repo: keystore y contraseñas por variables de entorno
  (`RNOTE_KEYSTORE`, `RNOTE_KEYSTORE_PASSWORD`, `RNOTE_KEY_ALIAS`, `RNOTE_KEY_PASSWORD`).
- Biblioteca Android en `filesDir/library`. Metadatos en `<raíz>/.rnote-library/library.json`,
  papelera en `<raíz>/.rnote-library/trash/`.
- Se listan `.rnote` y `.xopp` de la raíz y de carpetas de un nivel; se ignoran ocultos
  (nombre que empieza con `.`) y cualquier otra extensión.
- Commits en español `tipo: descripción`, uno por tipo; verificación en verde antes de cada
  commit: `cargo test -p <crate>`, `cargo clippy -p <crate> --all-targets -- -D warnings`
  en host, clippy de `rnote-android` para aarch64
  (`. android/scripts/env.sh aarch64 && cargo clippy -p rnote-android --target aarch64-linux-android -- -D warnings`),
  `android/scripts/build-apk.sh aarch64` y `./gradlew lintDebug` (en `android/viewer`) sin
  avisos nuevos respecto de: AndroidGradlePluginVersion, GradleDependency, OldTargetApi.
- Antes de instalar o lanzar en la tablet (`ANDROID_SERIAL` = el dispositivo de 无线调试):
  `adb shell dumpsys activity activities | grep -m1 topResumedActivity` en un comando
  aparte; si hay un juego u otra app en uso adelante, no lanzar (instalar sí).
- Push a `origin` (YX14ng/rnote) al cerrar la etapa.

## Review Focus

1. Nombres raros al crear/renombrar (con `/`, `\`, que empiezan con `.`, solo espacios,
   acentos y emoji, más de 120 caracteres): se rechazan los inválidos con un error claro,
   se recortan espacios, se aceptan acentos y emoji, se truncan a 120.
2. Colisiones de nombre al restaurar, mover, duplicar, importar o crear: nunca pisar, sufijo
   ` (2)`, ` (3)`…
3. `library.json` corrupto, de versión futura o ausente: la biblioteca abre igual; el
   corrupto se renombra a `library.json.bad` y se arranca vacío; nunca se pierde un `.rnote`.
4. Un `.rnote` dañado en la biblioteca: su tarjeta muestra tapa lisa (la miniatura falla sin
   tirar la app) y al abrirlo aparece el error de siempre.
5. Volver con `<` con cambios sin guardar o con el guardado fallando: se guarda antes de
   salir; si falla, el editor no se cierra y muestra el error.

---

### Task 1: Keystore estable y firma del APK

**Files:**
- Modify: `android/viewer/app/build.gradle.kts`
- Create: `android/scripts/create-keystore.sh`
- Modify: `android/README.md` (sección "Signing")

**Interfaces:**
- Produces: APKs debug y release firmados con el keystore de `RNOTE_KEYSTORE` cuando la
  variable existe; sin ella, debug usa la clave de debug y release falla con un mensaje.

- [ ] **Step 1:** `create-keystore.sh <ruta>`: llama a `keytool -genkeypair -v -keystore <ruta> -alias rnote -keyalg RSA -keysize 4096 -validity 10000`, pidiendo las contraseñas por terminal (nunca por argumentos); se niega si `<ruta>` ya existe o está dentro del repo. Al final imprime el SHA-1 (`keytool -list -v`) y recuerda hacer una copia de seguridad fuera de la PC.
- [ ] **Step 2:** En `build.gradle.kts`, un `signingConfigs.create("stable")` leído de las cuatro variables de entorno; `buildTypes.debug` y `buildTypes.release` lo usan si `RNOTE_KEYSTORE` está definida. Si `release` se pide sin ella, `error("RNOTE_KEYSTORE no está definido: ver android/README.md")`.
- [ ] **Step 3:** El usuario corre el script con la ruta `~/.config/rnote-android/release.jks` y exporta las variables (en `~/.config/rnote-android/env`, que `android/scripts/env.sh` carga si existe). Pedírselo; las contraseñas las escribe él.
- [ ] **Step 4: Verificar**: `android/scripts/build-apk.sh aarch64` y luego `$ANDROID_HOME/build-tools/*/apksigner verify --print-certs app/build/outputs/apk/debug/app-debug.apk` muestra el SHA-1 del keystore nuevo, no el de `~/.android/debug.keystore`.
- [ ] **Step 5: Reinstalación única en la tablet**, con ok explícito del usuario en ese momento: comprobar que no hay copia de recuperación (`adb shell run-as io.github.yx14ng.rnote.viewer ls files/`), `adb uninstall io.github.yx14ng.rnote.viewer`, instalar el APK nuevo. Los archivos guardados en Documents no se tocan.
- [ ] **Step 6: Commit** `chore: firma estable del APK con keystore por variables de entorno` (build.gradle.kts, script) y `docs: cómo crear el keystore y firmar` (README).

### Task 2: `rnote-library`, escaneo y nombres

**Files:**
- Create: `crates/rnote-library/Cargo.toml`, `src/lib.rs`, `src/scan.rs`, `src/names.rs`, `src/tests/mod.rs`, `src/tests/scan.rs`
- Modify: `Cargo.toml` (workspace: miembro y `rnote-library = { path = "crates/rnote-library" }` en `[workspace.dependencies]`)

**Interfaces:**
- Produces (en `lib.rs`):
  ```rust
  pub struct Library { /* root, meta, clock */ }
  pub type Clock = fn() -> i64; // milisegundos desde epoch
  pub fn system_clock() -> i64;
  impl Library {
      pub fn open(root: &Path) -> anyhow::Result<Self>;            // usa system_clock
      pub fn open_with_clock(root: &Path, clock: Clock) -> anyhow::Result<Self>;
      pub fn root(&self) -> &Path;
      pub fn notebooks(&self) -> anyhow::Result<Vec<Notebook>>;
      pub fn folders(&self) -> anyhow::Result<Vec<String>>;
  }
  #[derive(Debug, Clone, PartialEq, Serialize)]
  pub struct Notebook {
      pub path: String,            // relativa, con '/', p. ej. "Álgebra/Parcial 1.rnote"
      pub name: String,            // sin extensión
      pub folder: Option<String>,
      pub kind: NotebookKind,      // Rnote | Xopp, serializado "rnote" | "xopp"
      pub modified_ms: i64,
      pub size: u64,
      pub tags: Vec<String>,       // ids, vacío hasta la Task 3
  }
  ```
- Produces (en `names.rs`): `pub fn validate_name(name: &str) -> anyhow::Result<String>` (recorta, rechaza vacío, `/`, `\`, `.` inicial, `\0`; trunca a 120 caracteres) y `pub fn unique_file_name(dir: &Path, stem: &str, ext: &str) -> String` (`"x.rnote"`, luego `"x (2).rnote"`, …).

- [ ] **Step 1: Tests que fallan** en `src/tests/scan.rs` sobre un `tempdir` (usar `std::env::temp_dir()` + nombre único; sin crates nuevos de test):
  - `lists_rnote_and_xopp_in_root_and_one_folder_level`: crea `a.rnote`, `b.xopp`, `F/c.rnote`, `F/G/d.rnote`, `notas.txt`, `.x.autosave.xopp`; espera exactamente `a.rnote`, `b.xopp`, `F/c.rnote` con `folder` `None`, `None`, `Some("F")` y `kind` correcto.
  - `open_creates_the_library_dir_without_touching_notebooks`: tras `open`, existe `.rnote-library/` y los archivos siguen iguales (mismo tamaño).
  - `folders_lists_first_level_dirs_except_hidden`: `F`, `Álgebra` sí; `.rnote-library`, `.git` no.
  - `validate_name_rejects_and_trims` (Review Focus 1): `"  hola  "` → `"hola"`; `"a/b"`, `"a\\b"`, `".x"`, `"   "` → error; `"Análisis 😀"` ok; 200 `'a'` → 120.
  - `unique_file_name_appends_counter` (Review Focus 2): con `x.rnote` y `x (2).rnote` existentes devuelve `x (3).rnote`.
- [ ] **Step 2:** `cargo test -p rnote-library` → FAIL (no compila).
- [ ] **Step 3:** Implementar `scan.rs`, `names.rs` y `Library::open*`, `notebooks`, `folders` con esas firmas.
- [ ] **Step 4:** `cargo test -p rnote-library` → PASS; clippy host limpio.
- [ ] **Step 5: Commit** `feat: crate rnote-library con escaneo de cuadernos y nombres válidos`.

### Task 3: `rnote-library`, operaciones, etiquetas, papelera y `library.json`

**Files:**
- Create: `crates/rnote-library/src/meta.rs`, `src/ops.rs`, `src/trash.rs`, `src/tags.rs`, `src/tests/ops.rs`, `src/tests/meta.rs`

**Interfaces:**
- Consumes: Task 2.
- Produces (métodos de `Library`, todos persisten `library.json` antes de devolver):
  ```rust
  pub fn create_folder(&mut self, name: &str) -> anyhow::Result<String>;
  pub fn rename_folder(&mut self, folder: &str, new_name: &str) -> anyhow::Result<String>;
  pub fn delete_folder(&mut self, folder: &str) -> anyhow::Result<()>;   // su contenido va a la papelera
  pub fn rename(&mut self, path: &str, new_name: &str) -> anyhow::Result<String>;   // nueva ruta
  pub fn move_to(&mut self, path: &str, folder: Option<&str>) -> anyhow::Result<String>;
  pub fn duplicate(&mut self, path: &str) -> anyhow::Result<String>;
  pub fn new_notebook_path(&self, folder: Option<&str>, stem: &str) -> anyhow::Result<String>; // no crea el archivo
  pub fn trash(&mut self, path: &str) -> anyhow::Result<()>;
  pub fn trash_entries(&self) -> Vec<TrashEntry>;
  pub fn restore(&mut self, trash_id: &str) -> anyhow::Result<String>;
  pub fn delete_forever(&mut self, trash_id: &str) -> anyhow::Result<()>;
  pub fn empty_trash(&mut self) -> anyhow::Result<()>;
  pub fn tags(&self) -> Vec<Tag>;
  pub fn create_tag(&mut self, name: &str, color: u32) -> anyhow::Result<String>; // id
  pub fn update_tag(&mut self, id: &str, name: &str, color: u32) -> anyhow::Result<()>;
  pub fn delete_tag(&mut self, id: &str) -> anyhow::Result<()>;
  pub fn set_tags(&mut self, path: &str, tag_ids: &[String]) -> anyhow::Result<()>;

  #[derive(Debug, Clone, PartialEq, Serialize)]
  pub struct Tag { pub id: String, pub name: String, pub color: u32 }   // color ARGB
  #[derive(Debug, Clone, PartialEq, Serialize)]
  pub struct TrashEntry { pub id: String, pub name: String, pub original_path: String, pub deleted_at_ms: i64 }
  ```
- `library.json` v1 (formato fijo; la etapa 3 lo mezcla):
  ```json
  { "version": 1,
    "tags":          { "<id>":   { "name": "…", "color": 4294198070, "updated_at": 0, "deleted": false } },
    "notebook_tags": { "<ruta>": { "tags": ["<id>"], "updated_at": 0, "deleted": false } },
    "trash":         { "<id>":   { "original_path": "…", "deleted_at": 0, "updated_at": 0, "deleted": false } } }
  ```
  Las bajas ponen `deleted: true` (lápida) y actualizan `updated_at` con el `Clock`. El id de
  papelera es el nombre del archivo en `trash/`: `<deleted_at>-<nombre de archivo>`. El id de
  etiqueta: 16 hex de un hash de `(clock(), nombre)` con `RandomState` (sin crates nuevos).
  Renombrar, mover y duplicar actualizan/copian las entradas de `notebook_tags`. La escritura
  es atómica (archivo temporal + `rename`).

- [ ] **Step 1: Tests que fallan** (`src/tests/ops.rs`, `src/tests/meta.rs`, reloj fijo `|| 1_000`):
  - `rename_and_move_keep_tags`: etiqueta en `a.rnote`, renombrar a `b`, mover a `F`; `notebooks()` muestra `F/b.rnote` con la etiqueta.
  - `trash_and_restore_round_trip`: `trash("F/b.rnote")` → no está en `notebooks()`, `trash_entries()` tiene `original_path == "F/b.rnote"`; `restore` lo devuelve a `F/b.rnote`.
  - `restore_never_overwrites` (Review Focus 2): se restaura con un `F/b.rnote` nuevo existente → `F/b (2).rnote`; el existente intacto (mismo contenido).
  - `restore_recreates_a_deleted_folder`.
  - `delete_folder_trashes_its_notebooks`.
  - `delete_forever_and_empty_trash_remove_files`.
  - `duplicate_copies_bytes_and_tags` → `a (2).rnote`.
  - `delete_tag_leaves_a_tombstone_and_hides_it`: `tags()` no lo lista; el JSON tiene `deleted: true` y `updated_at: 1000`.
  - `corrupt_library_json_is_set_aside` (Review Focus 3): `library.json` = `"{nope"` → `open` ok, existe `library.json.bad`, `tags()` vacío, `.rnote` listados.
  - `future_version_is_set_aside`: `{"version": 99}` → igual que el anterior.
  - `json_round_trip_matches_the_v1_format`: crear etiqueta y papelera, leer el archivo con `serde_json::Value` y comprobar las claves de arriba.
- [ ] **Step 2:** `cargo test -p rnote-library` → FAIL.
- [ ] **Step 3:** Implementar `meta.rs` (tipos serde + carga/guardado atómico + apartado de corruptos), `ops.rs`, `trash.rs`, `tags.rs` con esas firmas.
- [ ] **Step 4:** `cargo test -p rnote-library` → PASS; clippy limpio.
- [ ] **Step 5: Commit** `feat: carpetas, etiquetas y papelera de la biblioteca con library.json`.

### Task 4: `rnote-library`, miniaturas e importación de `.xopp`

**Files:**
- Create: `crates/rnote-library/src/thumbnail.rs`, `src/import.rs`, `src/tests/thumbnail.rs`
- Modify: `crates/rnote-library/Cargo.toml` (`rnote-engine`, `futures`)

**Interfaces:**
- Produces:
  ```rust
  /// PNG de la primera página, `size_px` de ancho máximo.
  pub fn render_thumbnail(rnote_bytes: Vec<u8>, size_px: u32) -> anyhow::Result<Vec<u8>>;
  /// Clave de caché de la miniatura de `notebook`: estable mientras no cambien ruta, mtime y tamaño.
  pub fn thumbnail_key(notebook: &Notebook) -> String;   // 16 hex
  impl Library { pub fn import_xopp(&mut self, path: &str) -> anyhow::Result<String>; } // ruta del .rnote creado
  ```
- `render_thumbnail`: `EngineSnapshot::load_from_rnote_bytes` → `Engine::default().load_snapshot` → `Engine::generate_thumbnail(size_px, SelectionExportFormat::Png)` con `futures::executor::block_on`. Si `extract_thumbnail_content` abarca más que la primera página, recortar a sus límites.
- `import_xopp`: `EngineSnapshot::load_from_xopp_bytes(bytes, XoppImportPrefs::default())` → `save_as_rnote_bytes(stem)` → `unique_file_name(dir, stem, "rnote")` en la misma carpeta; el `.xopp` no se toca.

- [ ] **Step 1: Tests que fallan**:
  - `thumbnail_of_a_new_document_is_a_png`: bytes de un documento vacío (guardado con `Engine::default().save_as_rnote_bytes`) → empieza con `\x89PNG`, ancho ≤ 256.
  - `thumbnail_of_garbage_is_an_error` (Review Focus 4): `vec![1, 2, 3]` → `Err`, sin panic.
  - `thumbnail_key_changes_with_mtime`.
  - `import_xopp_writes_a_new_rnote_beside_it`: usar como fixture un `.xopp` chico generado en el test con `export_doc_as_xopp_bytes` de un documento con un trazo (no copiar archivos del usuario); resultado `x.rnote` existe, `x.xopp` igual que antes, y el `.rnote` abre con `load_from_rnote_bytes`.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** Implementar.
- [ ] **Step 4:** `cargo test -p rnote-library` → PASS; clippy limpio.
- [ ] **Step 5: Commit** `feat: miniaturas e importación de .xopp en la biblioteca`.

### Task 5: Puente JNI de la biblioteca y `LibraryStore` en Kotlin

**Files:**
- Create: `crates/rnote-android/src/library_bridge.rs` (lógica pura + tests), `crates/rnote-android/src/jni_bridge/library.rs`
- Modify: `crates/rnote-android/Cargo.toml`, `src/lib.rs`, `src/jni_bridge/mod.rs`
- Create: `android/viewer/app/src/main/kotlin/io/github/yx14ng/rnote/viewer/LibraryStore.kt`
- Modify: `NativeBridge.kt`

**Interfaces:**
- Consumes: Tasks 2–4.
- Produces (Rust): `pub(crate) fn handle_request(root: &Path, request: &str) -> anyhow::Result<String>`, con `#[serde(tag = "op", rename_all = "snake_case")] enum LibraryRequest { List, CreateFolder{name}, RenameFolder{folder,new_name}, DeleteFolder{folder}, Rename{path,new_name}, MoveTo{path,folder:Option<String>}, Duplicate{path}, NewNotebookPath{folder:Option<String>,stem}, Trash{path}, Restore{id}, DeleteForever{id}, EmptyTrash, CreateTag{name,color}, UpdateTag{id,name,color}, DeleteTag{id}, SetTags{path,tags}, ImportXopp{path}, UniqueName{folder:Option<String>,file_name} }`. `List` devuelve `{"notebooks":[…],"folders":[…],"tags":[…],"trash":[…]}` con las structs serializadas, y cada cuaderno con un campo extra `"key"` = `thumbnail_key(&notebook)`; el resto `{"path": "…"}`, `{"id": "…"}` o `{}`.
- Produces (JNI, `NativeBridge.kt`): `external fun libraryCall(root: String, request: String): String` (lanza `RuntimeException` con el mensaje del error) y `external fun libraryThumbnail(rnoteBytes: ByteArray, sizePx: Int): ByteArray?` (null si falla).
- Produces (Kotlin, `LibraryStore.kt`):
  ```kotlin
  data class Notebook(val path: String, val name: String, val folder: String?, val isXopp: Boolean, val modifiedMs: Long, val size: Long, val tags: List<String>, val key: String)
  data class LibraryTag(val id: String, val name: String, val color: Int)
  data class TrashItem(val id: String, val name: String, val originalPath: String, val deletedAtMs: Long)
  data class LibrarySnapshot(val notebooks: List<Notebook>, val folders: List<String>, val tags: List<LibraryTag>, val trash: List<TrashItem>)
  class LibraryStore(val root: File) {
      fun list(done: (Result<LibrarySnapshot>) -> Unit)
      fun call(request: JSONObject, done: (Result<JSONObject>) -> Unit)  // un hilo de fondo, done en el hilo principal
      fun file(path: String): File
      companion object { fun forApp(context: Context): LibraryStore }   // filesDir/library
  }
  ```

- [ ] **Step 1: Tests Rust que fallan** en `library_bridge.rs`: `list_returns_the_four_sections`, `rename_request_returns_the_new_path`, `unknown_op_is_an_error`, `bad_json_is_an_error`.
- [ ] **Step 2:** `cargo test -p rnote-android library_bridge` → FAIL.
- [ ] **Step 3:** Implementar `handle_request` y las dos funciones JNI siguiendo el patrón de `jni_bridge/document.rs` (`with_env` + `ThrowRuntimeExAndDefault`); `LibraryStore` con un `Executors.newSingleThreadExecutor()` y `Handler(Looper.getMainLooper())`, parseo con `org.json`.
- [ ] **Step 4:** Tests PASS; clippy host y aarch64; `build-apk.sh aarch64` compila.
- [ ] **Step 5: Commit** `feat: puente JNI de la biblioteca y LibraryStore`.

### Task 6: Pantalla de biblioteca: grilla, abrir y crear

**Files:**
- Create: `LibraryActivity.kt`, `NotebookGrid.kt`, `ThumbnailCache.kt` (en el paquete `io.github.yx14ng.rnote.viewer`)
- Modify: `AndroidManifest.xml` (`LibraryActivity` es el `MAIN/LAUNCHER`; `MainActivity` conserva el filtro `VIEW`), `MainActivity.kt`, `DocumentSaver.kt`, `strings.xml` (EN/ES), `res/drawable/ic_*` nuevos (stroke icons como los existentes)

**Interfaces:**
- Consumes: Task 5.
- Produces: `MainActivity.EXTRA_LIBRARY_PATH: String` (ruta relativa) y `MainActivity.EXTRA_NEW: Boolean`. `LibraryActivity` abre con `Intent(this, MainActivity::class.java).setData(Uri.fromFile(store.file(path))).putExtra(EXTRA_LIBRARY_PATH, path)`; para "Nuevo" agrega `EXTRA_NEW = true` con la ruta de `NewNotebookPath` (stem `R.string.untitled`: "Untitled" / "Sin título") y `MainActivity` crea el documento vacío y lo guarda ahí de inmediato.
- `ThumbnailCache(context, store)`: `fun load(notebook: Notebook, into: ImageView)`; archivo `cacheDir/thumbnails/<notebook.key>.png` (la clave viene de Rust en `List`). Genera en un pool de 2 hilos; tapa lisa mientras tanto o si falla (Review Focus 4).

- [ ] **Step 1:** `NotebookGrid` (un `GridView` con `BaseAdapter`; modo grilla con tarjetas de 3 columnas en vertical y 5 en horizontal, modo lista con filas): primera tarjeta "Nuevo" (borde punteado con `+`), luego tarjetas con miniatura, nombre de una línea con elipsis, fecha (`DateFormat.getDateInstance(SHORT)`) y botón ⌄ (menú, se conecta en la Task 7). Los `.xopp` muestran la etiqueta "Xournal++ · importar".
- [ ] **Step 2:** `LibraryActivity` en el tema Grafito: barra arriba con título de la sección, botón vista (grilla/lista), orden (fecha de modificación descendente por defecto, nombre ascendente); grilla debajo. Recarga la lista en `onResume` (así la miniatura se regenera al volver del editor, porque cambia mtime → clave).
- [ ] **Step 3:** `MainActivity`: con `EXTRA_LIBRARY_PATH` el título es el nombre del cuaderno; `DocumentSaver` guarda por `Uri.fromFile` (verificar que `DocumentFiles.write` con modo `"wt"` funciona con `file://`; si no, escribir con `FileOutputStream` cuando `uri.scheme == "file"`). Abierto por `VIEW` externo sigue como hoy.
- [ ] **Step 4:** Tocar un `.xopp` → `ImportXopp` → abre el `.rnote` creado.
- [ ] **Step 5: Verificar en la tablet** (instalar, chequear primer plano, lanzar): la app abre en la biblioteca vacía; "Nuevo" crea "Sin título" y abre el editor; dibujar, volver con el botón atrás del sistema (el `<` llega en la Task 8) y la tarjeta muestra la miniatura con el trazo; cambiar orden y vista; captura con `adb exec-out screencap`.
- [ ] **Step 6:** build, lint y clippy en verde. **Commit** `feat: pantalla de biblioteca con grilla, miniaturas y cuadernos nuevos`.

### Task 7: Columna izquierda, menú de cuaderno, selección e importación

**Files:**
- Create: `LibrarySidebar.kt`, `NotebookActions.kt`, `TagDialogs.kt`
- Modify: `LibraryActivity.kt`, `NotebookGrid.kt`, `strings.xml` (EN/ES)

**Interfaces:**
- Consumes: Tasks 5–6; `ColorPicker` existente para el color de etiqueta; `PanelOverlay`/`PanelViews` para menús.
- Produces: `sealed interface LibrarySection { All; Uncategorized; Folder(name); Tag(id); Trash }` y `fun LibrarySnapshot.filter(section: LibrarySection): List<Notebook>`.

- [ ] **Step 1:** `LibrarySidebar` (columna fija de 280 dp en horizontal; en vertical, cajón que se abre con un botón ☰ en la barra): Todas las notas, Sin categoría, Carpetas (lista + `+`), Etiquetas (punto de color + nombre, `+`), Papelera; cada fila con su cantidad; la sección actual resaltada con el acento. Mantener presionada una carpeta o etiqueta → renombrar / borrar (borrar carpeta no vacía pide confirmación y manda su contenido a la papelera).
- [ ] **Step 2:** Menú ⌄ de tarjeta (`NotebookActions`): renombrar (valida con el error de `validate_name`, Review Focus 1), mover a carpeta (lista con "Sin categoría"), etiquetas (casillas + "Nueva etiqueta"), duplicar, a la papelera. En la sección Papelera: restaurar, borrar definitivamente (confirma), y en la barra "Vaciar papelera" (confirma).
- [ ] **Step 3:** Selección múltiple desde el botón "Seleccionar" de la barra: casillas en las tarjetas, barra inferior con mover, etiquetar, a la papelera y cancelar.
- [ ] **Step 4:** "Importar" en la barra: `DocumentFiles.openIntent` (aceptar `.rnote` y `.xopp`), copiar los bytes a la carpeta actual con `UniqueName`; un `.xopp` importado se convierte con `ImportXopp`.
- [ ] **Step 5: Verificar en la tablet:** crear carpeta "Álgebra" y etiqueta "Parcial" roja; mover y etiquetar un cuaderno, ver las cantidades; papelera y restaurar con un nombre ocupado → `(2)`; importar un `.rnote` de Documents; capturas. Revisar `logcat` sin errores.
- [ ] **Step 6:** build, lint en verde. **Commit** `feat: carpetas, etiquetas, papelera, selección e importación en la biblioteca`.

### Task 8: Barra superior estilo StarNote en el editor

**Files:**
- Create: `EditorTopBar.kt`, `KeyShortcuts.kt`, `res/drawable/ic_back.xml`, `ic_expand_less.xml`, `ic_expand_more.xml`
- Rename/Modify: `ToolSidebar.kt` → `ToolRow.kt` (horizontal; misma API `onToolSelected`, `onToolOptions`, `onColorSelected`, `onColorEdit`, `onWidthTapped`, `showPalette`, `showWidth`, `formatWidth` en su companion; expone `tools: View` y `palette: View` para ubicarlos por separado)
- Modify: `EditorLayout.kt` (columna: barra arriba, debajo un `FrameLayout` con lienzo, mensaje y barra de selección; overlays sobre todo), `EditorMenu.kt` (queda solo `Action`; se borra el código de `Menu`), `MainActivity.kt` (sin `onCreateOptionsMenu`/`onOptionsItemSelected`; `onKeyShortcut` usa `KeyShortcuts`), `PanelViews.kt` (`PanelOverlay` anclado a una vista), `ToolOptionsPanel.kt`, `MenuPanel.kt`, `res/values/themes.xml` (`Theme.Rnote` hereda de `android:Theme.Material.NoActionBar`), `strings.xml` (EN/ES)

**Interfaces:**
- Consumes: `EditorMenu.Action`, `EditorControls`, paneles existentes, Task 6 (`EXTRA_LIBRARY_PATH`).
- Produces:
  ```kotlin
  internal class EditorTopBar(context: Context, tools: ToolRow, onAction: (EditorMenu.Action) -> Unit, onBack: () -> Unit) : LinearLayout {
      fun show(state: MenuState)          // título, "Unsaved", habilitado de undo/redo, zoom %, finalizar forma
      val moreButton: View                // ancla del menú
  }
  internal object KeyShortcuts { fun actionFor(keyCode: Int, ctrl: Boolean, shift: Boolean): EditorMenu.Action? }
  internal class PanelOverlay(context: Context, panel: View, panelWidthDp: Float, anchor: View, onDismiss: () -> Unit)
  ```
- Fila 1 (52 dp): `<`, título (tocar → renombrar si es de la biblioteca, vía `Rename`, y actualizar `saver.currentUri`), "Unsaved" chico debajo, espacio, finalizar forma (si aplica), zoom "100 %" (tocar = ajustar al ancho), `⋯` (abre `MenuPanel` anclado), flecha que pliega/despliega la fila 2.
- Fila 2 (56 dp): deshacer y rehacer a la izquierda; herramientas de `ToolRow` centradas; paleta y botón de grosor a la derecha.
- `KeyShortcuts`: Ctrl+Z deshacer, Ctrl+Shift+Z y Ctrl+Y rehacer, Ctrl+X/C/V, Ctrl+A seleccionar todo, Ctrl+N nuevo (en la biblioteca, carpeta actual), Ctrl+O abrir, Ctrl+S guardar.
- `PanelOverlay` ubica el panel debajo del ancla: `x = clamp(ancla.centroX − ancho/2, 8 dp, pantalla − ancho − 8 dp)`, `y = ancla.abajo + 6 dp` (coordenadas con `getLocationInWindow`).
- `<` (Review Focus 5): si hay cambios, `saver.save()` y cerrar solo cuando el guardado termina bien; si falla, quedarse con el error. Si el editor vino de un `VIEW` externo, `<` abre `LibraryActivity` y cierra.

- [ ] **Step 1:** `ToolRow` horizontal a partir de `ToolSidebar` (mismos íconos y comportamientos).
- [ ] **Step 2:** `EditorTopBar` y el nuevo `EditorLayout`; tema sin ActionBar; `MainActivity` conectado.
- [ ] **Step 3:** `KeyShortcuts` + `onKeyShortcut`; borrar el código de `Menu` de `EditorMenu`.
- [ ] **Step 4:** `PanelOverlay` anclado; los paneles de herramientas aparecen bajo la herramienta tocada y el menú bajo `⋯`.
- [ ] **Step 5: Verificar en la tablet:** capturas de la barra con las dos filas; plegar/desplegar; cada herramienta y su panel; paleta y grosor; deshacer/rehacer; `adb shell input keycombination CTRL_LEFT Z` deshace; `<` con cambios guarda y vuelve a la biblioteca con la miniatura nueva; rueda del S Pen y deshacer con dos dedos siguen andando (pedir al usuario que lo pruebe). `logcat` sin errores.
- [ ] **Step 6:** build, lint en verde. **Commits** `feat: barra superior de dos filas estilo StarNote con vuelta a la biblioteca` y, si el cambio de `ToolSidebar` a `ToolRow` se puede separar limpio antes, `refactor: barra de herramientas horizontal` primero.

### Task 9: Cierre de la etapa

**Files:**
- Modify: `android/README.md` (biblioteca, barra nueva, firma), `android/viewer/app/build.gradle.kts` (`versionCode 3`, `versionName "0.2.0"`)

- [ ] **Step 1:** Actualizar el README con lo nuevo y lo que se sacó (barra lateral, ActionBar).
- [ ] **Step 2:** Verificación completa: `cargo test -p rnote-library -p rnote-android`, clippy host y aarch64, `build-apk.sh aarch64`, `lintDebug`.
- [ ] **Step 3: Commits** `docs: biblioteca y barra estilo StarNote en el README` y `chore: versión 0.2.0`. `git push origin android/engine-build`.
- [ ] **Step 4:** Informar al usuario: qué probar a mano (rueda, dos dedos, palma con la barra nueva) y que el release necesita su "publicalo".
