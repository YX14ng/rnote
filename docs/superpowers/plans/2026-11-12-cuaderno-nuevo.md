# Cuaderno nuevo estilo StarNote — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** En la biblioteca de Android, el "+" ofrece Cuaderno nuevo…, Nota rápida, Nota
infinita, Importar PDF…, Importar imagen y Sacar foto. Los cuadernos pueden tener una portada
de color que se ve en la tablet y en el escritorio y viaja con la sincronización. La última
configuración queda guardada por carpeta. Un PDF de cientos de páginas se importa, se abre y
sube a Drive.

**Architecture:**
- Todo lo que se puede testear va en Rust:
  - `rnote-library`: portadas en `library.json` y claves desconocidas conservadas.
  - `rnote-sync`: subida reanudable para archivos de más de 5 MB.
  - `rnote-android`: arma los bytes del `.rnote` nuevo a partir de una configuración (en
    blanco, con imagen o con PDF página por página), y guarda la última configuración por
    carpeta en un JSON del dispositivo.
- Kotlin dibuja el menú y las pantallas con vistas a mano en `Graphite`. Escribe el archivo
  en la biblioteca solo cuando los bytes están completos y después abre el cuaderno en el
  editor como uno existente.

**Tech Stack:** Rust 2024 del workspace; `hayro` 0.8 (ya en el workspace) para contar páginas
y detectar contraseña; `image` 0.25 del workspace para orientar y achicar fotos; `jni` 0.22;
Kotlin sin AndroidX propio; gtk4-rs en `rnote-ui` para las tarjetas del escritorio.

**Spec:** `docs/superpowers/specs/2026-11-12-cuaderno-nuevo-design.md`. Diseño visual: canvas
https://claude.ai/artifact/PFWV4m2m1e2qjVVaq3qwcK (artboards "Biblioteca: menú del +",
"Cuaderno nuevo", "Importar PDF"). Cuando el canvas y el spec discrepan, manda el spec.

## Global Constraints

- Plantillas: En blanco, Renglones, Cuadrícula, Puntos (`PatternStyle` None, Lines, Grid,
  Dots). Interlineado: 4, 5, 6, 8, 10 mm, convertidos con `format.dpi()` como
  `apply_page_size`.
- Color del papel: los cinco de `BackgroundColor` (`DocumentFormat.kt:80`). El patrón usa
  `BackgroundColor.patternColor`.
- Páginas: Hojas = `Layout::FixedSize`, Continua = `ContinuousVertical`, Infinita =
  `Infinite`.
- Portadas, ocho colores (ARGB): Azul #3D6FB6, Verde #3F8F6B, Naranja #C9773A, Rojo #B5524E,
  Violeta #7A5BB0, Turquesa #2F8C93, Mostaza #B8922E, Grafito #5E6470.
- Sin nada guardado para la carpeta: A4 vertical, Hojas, Oscuro, Cuadrícula de 5 mm, sin
  portada.
- `library.json` sigue en `version` 1. Un archivo sin `notebook_covers` se lee como sin
  portadas.
- PDF: Vector por defecto; Imagen a 1,8× (`bitmap_scalefactor` del motor). La hoja toma el
  ancho del formato y el alto de la página más alta, en `FixedSize`.
- Subida reanudable para archivos de más de 5 MiB (`5 * 1024 * 1024` bytes); hasta ese tamaño,
  como hoy.
- Ningún `.rnote` a medias en la biblioteca: el archivo se escribe una sola vez, con los
  bytes completos.
- Rust nuevo: Google Rust Style Guide, mismos `[lints]` de cada crate, tests como módulos;
  upstream no se reformatea. Kotlin nuevo con el estilo de los archivos vecinos.
- Archivos de 200–400 líneas típico, 800 tope. `LibraryActivity.kt` (335) y `MainActivity.kt`
  (561) solo reciben el cableado.
- Textos de UI en `values/strings.xml` (inglés) y `values-es/strings.xml` (español),
  copiando las palabras del canvas.
- No abrir ventanas, navegadores ni apps gráficas en la PC: el escritorio se verifica con
  tests y compilación, no a ojo.
- Tablet: chequear `topResumedActivity` en un comando aparte justo antes de cada
  instalación o lanzamiento. No instalar ni lanzar si el usuario está en otra app o en Rnote.

## Review Focus

1. **Un `library.json` sin portadas, de otra versión,** se une con uno que las tiene. Las
   portadas quedan, y una clave que esta versión no conoce sobrevive a cargar y guardar
   (tarea 1: `merge_keeps_covers_from_a_copy_without_them`,
   `unknown_keys_survive_load_and_save`, `merge_keeps_unknown_keys_of_both_sides`).
2. **PDF con páginas verticales y horizontales mezcladas.** Cada página ocupa el ancho de la
   hoja, ninguna se superpone, y cada una arranca en su propia hoja (tarea 4:
   `mixed_page_sizes_fill_the_width_one_per_page`).
3. **PDF con contraseña.** Sin contraseña, o con una equivocada, se vuelve a pedir; no hay un
   error genérico ni un crash (tarea 4: `encrypted_pdf_needs_a_password`,
   `wrong_password_needs_a_password`, `right_password_opens`).
4. **Foto de la cámara del Samsung,** con orientación EXIF y 12 MP. Queda derecha y no pesa
   lo que la foto original (tarea 3: `photo_is_turned_upright`, `big_photo_is_scaled_down`).
5. **Subida justo en 5 MiB, o la sesión reanudable que falla:** exactamente 5 MiB va por el
   camino de hoy. Un error en la sesión se reporta como cualquier otro fallo de Drive y la
   sincronización queda pendiente (tarea 2: `upload_of_exactly_5_mib_is_multipart`,
   `resumable_upload_failure_is_an_error`).

## Verificación (cada tarea Rust la corre entera)

```bash
cd ~/Projects/rnote && export CARGO_HOME=$PWD/_mesonbuild/cargo-home
cargo test -p <crate> && cargo clippy -p <crate> --all-targets -- -D warnings && cargo fmt -p <crate> -- --check
# rnote-android además:
(. android/scripts/env.sh aarch64 && cargo clippy -p rnote-android --target aarch64-linux-android -- -D warnings)
```

APK y lint (tareas 5–6):
```bash
. ~/.config/rnote-android/signing.env && ANDROID_HOME=$HOME/.cache/rnote-android/sdk \
  JAVA_HOME=/usr/lib/jvm/java-21-openjdk android/scripts/build-apk.sh aarch64
(cd android/viewer && ./gradlew -q lintDebug)  # app/build/reports/lint-results-debug.txt: 0 errores
```

Las salidas largas van a un log del workspace del plan y se lee la cola.

---

### Tarea 1: portadas en la biblioteca y claves desconocidas conservadas

**Files:**
- Modify: `crates/rnote-library/src/meta.rs` (`CoverRecord`, `notebook_covers`, `extra`,
  `merge_meta`, operaciones por cuaderno)
- Create: `crates/rnote-library/src/covers.rs` (API pública de portadas)
- Modify: `crates/rnote-library/src/lib.rs` (`Notebook.cover`, `notebooks()`, `mod covers`)
- Modify: `crates/rnote-library/src/ops.rs:39,96,117,131`, `trash.rs:72,111,134`,
  `import.rs:35` (usar las operaciones nuevas)
- Modify: `crates/rnote-android/src/library_bridge.rs` (`SetCover`)
- Test: `crates/rnote-library/src/tests/meta.rs`, `tests/ops.rs`, nuevo `tests/covers.rs`;
  tests de `library_bridge.rs`

**Interfaces:**
- Produces:
  - `Notebook.cover: Option<u32>` (ARGB). Se serializa como `"cover": null | número`.
  - `Library::set_cover(&mut self, path: &str, color: Option<u32>) -> anyhow::Result<()>`.
    `None` quita la portada; un path que no es cuaderno es error, como `set_tags`.
  - Pedido JSON `{"op":"set_cover","path":…,"color":null|n}` → `{}`.
  - Dentro del crate:
    - `Meta::move_notebook(&mut self, from, to, now)`: mueve etiquetas y portada.
    - `Meta::forget_notebook(&mut self, path, now)`: deja lápidas de las dos.
    - `Meta::copy_notebook(&mut self, from, to, now)`: copia etiquetas y portada; la usan
      `duplicate` e `import_xopp`.
    - Estas tres reemplazan a `move_tags` y `forget_tags` en todos sus usos.

- [ ] **Step 1: tests que fallan**
  - En `tests/covers.rs`:
    - `set_cover_shows_on_the_notebook`: `set_cover("A/x.rnote", Some(0xFF3D6FB6))`, y
      `notebooks()` lo da con `cover == Some(0xFF3D6FB6)`. `set_cover(.., None)` lo deja en
      `None`.
    - `cover_follows_rename_move_folder_rename_and_trash`: después de `rename`, `move_to`,
      `rename_folder`, `trash` y `restore`, el cuaderno sigue con su color.
    - `duplicate_and_xopp_import_copy_the_cover`.
    - `delete_forever_leaves_a_tombstone`: tras `delete_forever`, el registro de
      `notebook_covers` en `library.json` está con `"deleted": true`.
  - En `tests/meta.rs`:
    - `merge_keeps_covers_from_a_copy_without_them`: local sin la clave `notebook_covers`
      (JSON escrito a mano, como lo guarda la versión anterior) y remoto con una portada; el
      resultado tiene la portada, y lo mismo con los lados invertidos.
    - `merge_picks_the_newer_cover`: la de `updated_at` mayor gana, y la lápida también.
    - `unknown_keys_survive_load_and_save`: `{"version":1,"futuro":{"a":1}}` se carga y se
      guarda, y `"futuro"` sigue igual.
    - `merge_keeps_unknown_keys_of_both_sides`: local `{"x":1}` y remoto `{"x":2,"y":3}` dan
      `x == 1` y `y == 3`.
  - En los tests de `library_bridge.rs`: `set_cover_request_sets_the_cover`.

- [ ] **Step 2:** `cargo test -p rnote-library` → FALLA al compilar: no existen `set_cover` ni
  `Notebook.cover`.

- [ ] **Step 3: implementar.**
  - `CoverRecord { color: u32, updated_at: i64, #[serde(default)] deleted: bool }`.
  - `Meta.notebook_covers: BTreeMap<String, CoverRecord>` con `#[serde(default)]`.
  - `Meta.extra: serde_json::Map<String, Value>` con `#[serde(flatten)]`; también en `Default`.
  - `merge_meta`: une las portadas con `merge_map` y `extra` por clave, con la local ganando.
  - `impl_record!` suma `CoverRecord`.

- [ ] **Step 4:** correr la verificación de `rnote-library` y la de `rnote-android` → todo en
  verde. Los tests que ya existían de etiquetas siguen pasando sin cambios.

- [ ] **Step 5: commit:** `feat: portadas de color en la biblioteca, sin perder claves que no conoce`

### Tarea 2: subida reanudable a Drive para archivos de más de 5 MiB

**Files:**
- Modify: `crates/rnote-sync/src/drive.rs`. `create_file` y `update_file` eligen el camino por
  tamaño. `send` se separa para aceptar una URL absoluta y devolver también los encabezados.
- Modify: `crates/rnote-sync/src/tests/stub_http.rs`, para que una respuesta lleve
  encabezados extra (hace falta `Location`).
- Test: `crates/rnote-sync/src/tests/drive.rs`

**Interfaces:**
- Produces: `const RESUMABLE_THRESHOLD: usize = 5 * 1024 * 1024;` en `drive.rs`. `Remote` no
  cambia, así que el resto de `rnote-sync` no se entera.

El flujo, según la documentación de Drive v3:
1. Se inicia la sesión con `POST /upload/drive/v3/files?uploadType=resumable&fields=…` (para
   crear, con body JSON `{"name","parents"}`) o con `PATCH
   /upload/drive/v3/files/{id}?uploadType=resumable&fields=…` (para actualizar, sin body).
   Las dos llevan el encabezado `X-Upload-Content-Type: application/octet-stream`.
2. Drive responde 200 con `Location: <url de sesión>`.
3. Un solo `PUT <url de sesión>` con todos los bytes. La respuesta 200/201 trae el `FileJson`.

El token y los reintentos ante 429/5xx son los mismos de `send`. Si falta `Location`, es
`RemoteError::Failed`.

- [ ] **Step 1: tests que fallan**
  - `upload_of_exactly_5_mib_is_multipart`: `create_file` con `vec![0; 5 * 1024 * 1024]`
    hace un solo pedido con `uploadType=multipart`.
  - `big_create_uses_a_resumable_session`: con 5 MiB + 1 byte hay dos pedidos.
    - El primero: `POST`, `uploadType=resumable`, body JSON con `name` y `parents`.
    - El segundo: `PUT` a la ruta del `Location` del stub, con el body igual a los bytes.
    - Resultado: el `RemoteFile` del segundo.
  - `big_update_uses_a_resumable_session`: lo mismo con `PATCH /upload/drive/v3/files/n`.
  - `resumable_upload_failure_is_an_error`: el `PUT` responde 400, y el resultado es
    `Err(RemoteError::Failed(_))`.
  - `resumable_start_without_location_is_an_error`.

- [ ] **Step 2:** `cargo test -p rnote-sync drive` → FALLAN los cuatro tests de archivos
  grandes (hoy va un solo pedido multipart o media). El test de 5 MiB exactos pasa: confirma
  el borde.

- [ ] **Step 3:** implementar según el flujo de arriba.

- [ ] **Step 4:** verificación de `rnote-sync` → en verde. Los 92 tests anteriores siguen
  pasando.

- [ ] **Step 5: commit:** `feat: los archivos de más de 5 MB suben a Drive con subida reanudable`

### Tarea 3: armar un cuaderno nuevo desde una configuración (en blanco o con imagen)

**Files:**
- Create: `crates/rnote-android/src/notebook_setup.rs`
- Create: `crates/rnote-android/src/notebook_defaults.rs`
- Create: `crates/rnote-android/src/jni_bridge/creation.rs`; registrar en `jni_bridge/mod.rs`
- Modify: `crates/rnote-android/Cargo.toml`: `image = { workspace = true }` (y `hayro`, para
  la tarea 4)
- Modify: `android/viewer/app/src/main/kotlin/.../NativeBridge.kt`
- Test: `crates/rnote-android/src/notebook_setup/tests.rs` y `notebook_defaults/tests.rs`
  (módulos `#[cfg(test)]`)

**Interfaces:**
- Produces:
  - Rust:
    - `NotebookSetup` (serde, snake case), campos:
      - `page_width_mm: f64`, `page_height_mm: f64`
      - `layout: SetupLayout`: `fixed_size` | `continuous_vertical` | `infinite`
      - `background_argb: u32`, `pattern: SetupPattern` (`none` | `lines` | `grid` | `dots`)
      - `pattern_argb: u32`, `pattern_size_mm: f64`
      - `cover: Option<u32>`
    - `impl Default for NotebookSetup`: A4 210×297 vertical, `fixed_size`, background
      0xFF282A36, `grid`, pattern 0xFF6272A4, 5 mm, `cover: None`.
    - `pub(crate) fn new_notebook_bytes(setup: &NotebookSetup, file_name: &str) ->
      anyhow::Result<Vec<u8>>`: aplica la configuración con `apply_page_size`, `apply_layout`
      y `apply_background`, y guarda.
    - `pub(crate) fn image_notebook_bytes(setup: &NotebookSetup, image: &[u8], file_name: &str)
      -> anyhow::Result<Vec<u8>>`: la imagen arriba de la primera hoja, ajustada al ancho de
      la hoja, derecha según EXIF, con el lado mayor achicado a 2400 px como máximo
      (se le pasa al motor como PNG ya derecho y achicado).
    - `pub(crate) fn engine_with_setup(setup: &NotebookSetup) -> Engine`, para que la use la
      tarea 4.
    - `notebook_defaults.rs`:
      - `pub(crate) fn defaults_for(file: &Path, folder: Option<&str>) -> NotebookSetup`.
        Archivo inexistente, ilegible o sin la carpeta → `NotebookSetup::default()`.
      - `pub(crate) fn remember(file: &Path, folder: Option<&str>, setup: &NotebookSetup) ->
        anyhow::Result<()>`: escribe un JSON `{ "<carpeta o vacío>": setup }`, reemplazando el
        archivo de una vez.
  - JNI (`NativeBridge`):
    - `newNotebook(setupJson: String, fileName: String): ByteArray`
    - `imageNotebook(setupJson: String, image: ByteArray, fileName: String): ByteArray`
    - `notebookDefaults(file: String, folder: String?): String` (el JSON del setup)
    - `rememberNotebookDefaults(file: String, folder: String?, setupJson: String)`
    - El archivo es `filesDir/notebook_defaults.json`, fuera de la biblioteca, así que no se
      sincroniza.

- [ ] **Step 1: tests que fallan** (abren los bytes con `RnoteDocument::try_open` y leen
  `document_settings`):
  - `default_setup_is_a4_dark_grid_5mm`: ancho y alto en mm ≈ 210/297 (tolerancia 0,01),
    `Layout::FixedSize`, patrón Grid, `pattern_size` ≈ 5 mm en px (5 / 25,4 × dpi).
  - `setup_is_applied`: Carta horizontal, `infinite`, `lines`, 8 mm y crema; leídos de vuelta,
    son esos valores.
  - `image_notebook_puts_the_image_at_the_top_full_width`: un PNG de 400×200 hecho en el test
    con `image`. Hay un solo trazo `BitmapImage`; sus bounds empiezan en (0,0), con ancho =
    ancho de la hoja y alto = la mitad.
  - `photo_is_turned_upright`: JPEG de 40×20 con EXIF orientation 6 (hecho en el test: encoder
    JPEG de `image` y un segmento APP1 mínimo insertado a mano). La imagen del cuaderno queda
    más alta que ancha.
  - `big_photo_is_scaled_down`: JPEG de 4000×3000; la imagen guardada tiene 2400 px en el
    lado mayor.
  - `defaults_are_remembered_per_folder`: `remember` para "Álgebra" y para la raíz (`None`);
    cada una devuelve lo suyo, y "Física" devuelve `default()`.
  - `broken_defaults_file_gives_the_default`.

- [ ] **Step 2:** `cargo test -p rnote-android notebook_` → FALLA al compilar (no existen los
  módulos).

- [ ] **Step 3:** implementar lo de Interfaces. La imagen se inserta con el mismo
  `BitmapImage::from_image_bytes` que usa `paste_image`, con
  `ImageSizeOption::ImposeSize(ancho de la hoja, proporcional)`, en la capa `Document`.

- [ ] **Step 4:** verificación completa de `rnote-android`, host y aarch64 → en verde.

- [ ] **Step 5: commit:** `feat: armar un cuaderno nuevo con formato, papel, plantilla o una imagen`

### Tarea 4: importar un PDF página por página, con contraseña

**Files:**
- Create: `crates/rnote-android/src/pdf_import.rs` y `pdf_import/tests.rs`
- Create: `crates/rnote-android/src/tests/fixtures/` con `tres_paginas.pdf` (A4 vertical, A4
  horizontal, A4 vertical, con texto) y `con_clave.pdf` (el mismo, cifrado con contraseña de
  usuario `clave`). Se generan una vez con un script de Python que escribe el PDF mínimo a mano,
  y `qpdf` arregla el xref (`qpdf in.pdf tres_paginas.pdf`) y cifra (`qpdf --encrypt clave
  dueño 256 -- tres_paginas.pdf con_clave.pdf`). El script no se commitea; los PDF sí.
- Modify: `crates/rnote-android/src/jni_bridge/creation.rs`, `NativeBridge.kt`

**Interfaces:**
- Consumes: `NotebookSetup` y `engine_with_setup` (tarea 3).
- Produces:
  - Rust:
    - `pub(crate) enum PdfOpenError { NeedsPassword, Invalid(anyhow::Error) }`
    - `pub(crate) fn pdf_page_count(bytes: &[u8], password: Option<&str>) -> Result<usize,
      PdfOpenError>`, con `hayro::hayro_syntax::Pdf`.
      `LoadPdfError::Decryption(DecryptionError::PasswordProtected)` → `NeedsPassword`.
    - `pub(crate) struct PdfImport`:
      - `PdfImport::new(bytes: Vec<u8>, password: Option<String>, setup: &NotebookSetup,
        pages: Range<usize>, vector: bool) -> Result<Self, PdfOpenError>`. Un rango vacío o
        que sale del PDF es `Invalid`.
      - `fn total(&self) -> usize`
      - `fn import_next(&mut self) -> anyhow::Result<bool>`: importa una página; `false`
        cuando no quedan.
      - `fn finish(self, file_name: &str) -> anyhow::Result<Vec<u8>>`
  - JNI:
    - `pdfPageCount(bytes: ByteArray, password: String?): Int`: devuelve `-1` si necesita
      contraseña, y tira en un PDF inválido.
    - `pdfImportBegin(bytes, password: String?, setupJson, firstPage: Int, endPage: Int,
      vector: Boolean): Long`
    - `pdfImportTotal(handle): Int`, `pdfImportNext(handle): Boolean`,
      `pdfImportFinish(handle, fileName): ByteArray`.
    - `pdfImportClose(handle)` libera siempre, también al cancelar. Mismo patrón de handles
      que `open` y `close`.

**Algoritmo** (no lo determinan las firmas):
- Cada `import_next` llama a `VectorImage::from_pdf_bytes` (o `BitmapImage::` si
  `vector == false`) con `page_range = i..i+1` y prefs:
  - `adjust_document: true`, `page_spacing: OnePerDocumentPage`, `bitmap_scalefactor: 1.8`
  - `insert_pos = (0, y)`, con `y` la suma de las alturas de hoja de las páginas anteriores.
- Así cada página ocupa el ancho del formato (el zoom es por página).
- Al importar cada página:
  - Se guarda su altura (`bounds().extents()`).
  - Se inserta en la capa `Document` con `store.insert_stroke`.
- `finish`:
  - El alto del formato es la página más alta.
  - Se reubica cada página en `y = i × alto` (desplazando el trazo por la diferencia).
  - Layout `FixedSize` y `doc_resize_to_fit_content()`, después guardar.

- [ ] **Step 1: tests que fallan**
  - `page_count_of_a_plain_pdf`: 3.
  - `encrypted_pdf_needs_a_password`: `con_clave.pdf` sin clave → `NeedsPassword`.
  - `wrong_password_needs_a_password`: `"otra"` → `NeedsPassword`.
  - `right_password_opens`: `"clave"` → 3.
  - `garbage_is_invalid`: `b"no soy un pdf"` → `Invalid`.
  - `import_reports_each_page_then_finishes`: `import_next` devuelve `true`, `true`, `true`,
    `false`; los bytes abren con `try_open` y tienen 3 imágenes, de tipo `VectorImage`.
  - `bitmap_import_makes_bitmap_images`: `vector: false` da 3 `BitmapImage`.
  - `mixed_page_sizes_fill_the_width_one_per_page`:
    - Cada imagen tiene ancho = ancho de la hoja (±0,5).
    - La imagen `i` arranca en `y = i × alto_del_formato` y no se superpone con la siguiente.
    - El alto del formato es la página vertical.
    - El layout es `FixedSize`.
  - `page_range_imports_only_those_pages`: `1..2` → una imagen, la horizontal.
  - `page_range_outside_the_pdf_is_invalid`: `2..5` → `Invalid`.

- [ ] **Step 2:** `cargo test -p rnote-android pdf_` → FALLA al compilar.

- [ ] **Step 3:** implementar según Interfaces y Algoritmo.

- [ ] **Step 4:** verificación completa de `rnote-android`, host y aarch64 → en verde.

- [ ] **Step 5: commit:** `feat: importar un PDF como cuaderno, página por página y con contraseña`

### Tarea 5: el "+" de la biblioteca, la pantalla Cuaderno nuevo y las portadas en la tablet

**Files (todo bajo `android/viewer/app/src/main/kotlin/io/github/yx14ng/rnote/viewer/`):**
- Create: `NotebookSetup.kt`
  - `data class NotebookSetup`, espejo del JSON de la tarea 3, con `toJson()` y
    `fromJson()`.
  - `enum class CoverColor(argb, label)` con los ocho colores.
  - `enum class Template` con las cuatro plantillas.
  - `SPACINGS_MM = listOf(4.0, 5.0, 6.0, 8.0, 10.0)`.
- Create: `NotebookCreator.kt`
  - `create(folder: String?, title: String, setup: NotebookSetup, tags: List<String>, done:
    (Result<String>) -> Unit)`. En el hilo de `LibraryStore`:
    1. `new_notebook_path(folder, stem = título o R.string.untitled)`.
    2. `NativeBridge.newNotebook`.
    3. Escribe el archivo; si ya existe, es error y no lo pisa.
    4. `set_cover` si hay portada, y `set_tags`.
    5. `rememberNotebookDefaults`.
    6. Devuelve el path.
  - `quick(folder, layoutOverride: SetupLayout? = null, done)`: usa `notebookDefaults(folder)`
    sin pantalla. La Nota infinita pasa `infinite`.
  - `fromImage(folder, name, imageBytes, done)`: igual, con `imageNotebook`.
- Create: `CreateMenu.kt`: el popup del "+" con los seis items del artboard "Biblioteca: menú
  del +", con íconos y subtítulos.
- Create: `NewNotebookScreen.kt` (y `NewNotebookOptions.kt` si pasa de 400 líneas): la
  pantalla completa del artboard "Cuaderno nuevo", como `Dialog` a pantalla completa con
  vistas a mano.
  - Barra Cancelar / título / Crear.
  - Vista previa de la portada y de la hoja.
  - Campos: título, carpeta, etiquetas (reusa `TagDialogs`) y portada (interruptor y ocho
    colores).
  - Formato: los `PageSize` del editor, más vertical u horizontal.
  - Páginas (segmentado), color del papel (cinco), interlineado (cinco) y plantilla (cuatro,
    con vista previa).
  - El texto de ayuda del pie.
  - Arranca con `notebookDefaults(carpeta actual)`.
- Modify: `LibraryStore.kt`: `Notebook.cover: Int?`, leído de `"cover"`.
- Modify: `NotebookGrid.kt`: con `cover != null`, la tarjeta muestra una tapa del color con
  lomo y el nombre en una etiqueta clara, como la vista previa del canvas, en vez de la
  miniatura.
- Modify: `NotebookActions.kt`: en el "⋮" del cuaderno, item "Portada…" con los ocho colores
  más "Sin portada", que llama a `set_cover`.
- Modify: `LibraryActivity.kt`: la tarjeta "Nuevo" abre `CreateMenu`, que reemplaza a
  `createNotebook()`. Crear abre el editor con `startEditor(path, isNew = false)`.
- Modify: `MainActivity.kt`: "Nuevo" del menú del editor = Nota rápida en la carpeta del
  cuaderno abierto (o la raíz), y abre el resultado. El camino de `EXTRA_NEW` sigue para los
  intents que ya lo usan.
- Modify: `res/values/strings.xml`, `res/values-es/strings.xml`

- [ ] **Step 1:** implementar y compilar el APK → `BUILD SUCCESSFUL`. Lint → 0 errores, y
  ninguna advertencia nueva respecto de las 5 de base.
- [ ] **Step 2:** `apksigner verify --print-certs` → SHA-1
  `da2b01c2cc98827409bd131579b0cdf34b759dbc`. `llvm-nm -D` de la `.so` exporta
  `Java_..._newNotebook`, `_imageNotebook`, `_notebookDefaults` y `_rememberNotebookDefaults`.
- [ ] **Step 3: commit:** `feat: menú del + y pantalla de cuaderno nuevo con portadas en la tablet`

Kotlin no tiene tests unitarios en este proyecto. Lo que se puede testear vive en las tareas
1, 3 y 4; esto se prueba en la tablet (tarea 8).

### Tarea 6: importar PDF, imagen y foto desde la biblioteca

**Files (mismo directorio):**
- Create: `ImportPdfScreen.kt`: el artboard "Importar PDF".
  - Archivo, páginas y tamaño.
  - Nombre, carpeta y ancho de la hoja (los `PageSize`).
  - Páginas: Todas o Desde–hasta, con números del 1 al total, validados antes de habilitar
    Importar.
  - Tipo: Vector (Recomendado) o Imagen, con los textos del canvas.
  - El texto de la goma.
- Create: `PdfImporter.kt`
  - Lee los bytes del `Uri`.
  - Llama a `pdfPageCount`. Con `-1` pide la contraseña en un `FormDialog`; si es equivocada,
    la vuelve a pedir con el mensaje "Contraseña incorrecta".
  - Después de "Importar", en un hilo propio: `pdfImportBegin`, un `pdfImportNext` por página
    que publica "Página n de total" en el diálogo de progreso (barra y Cancelar), y
    `pdfImportFinish`. Recién ahí escribe el archivo con `unique_name` y abre el editor.
  - Cancelar marca un flag que se mira entre páginas, y `pdfImportClose` va en un `finally`:
    no se escribe nada.
  - El nombre por defecto es el del PDF sin extensión.
- Create: `PhotoCapture.kt`: `ACTION_IMAGE_CAPTURE` con `EXTRA_OUTPUT` a
  `cacheDir/camera/foto.jpg` vía `FileProvider`. Al volver con `RESULT_OK`, llama a
  `NotebookCreator.fromImage`; si no, no pasa nada. Nombre: "Foto" + fecha `yyyy-MM-dd HH.mm`.
- Modify: `AndroidManifest.xml`: `<provider android:name="android.content.FileProvider"
  android:authorities="${applicationId}.files" android:exported="false"
  android:grantUriPermissions="true">` con `res/xml/file_paths.xml` (`<cache-path
  name="camera" path="camera/"/>`).
  - Si `android.content.FileProvider` no existe sin AndroidX, se arma un `ContentProvider`
    mínimo propio (`CameraFileProvider.kt`, solo `openFile` del archivo de la cámara) y se
    deja la decisión en el ledger.
  - No se pide el permiso `CAMERA`.
- Modify: `CreateMenu.kt` / `LibraryActivity.kt`.
  - Importar PDF abre el selector con `application/pdf`; Importar imagen, con `image/*`.
  - Sacar foto: `PhotoCapture`.
  - Los resultados vuelven por `onActivityResult`, como `LibraryImport`.
- Modify: `strings.xml` (los dos)

- [ ] **Step 1:** implementar, compilar el APK y correr lint → `BUILD SUCCESSFUL`, 0 errores,
  sin advertencias nuevas.
- [ ] **Step 2:** `aapt2 dump xmltree` del manifiesto del APK muestra el provider y no tiene
  `android.permission.CAMERA`. `llvm-nm -D` exporta los cinco `pdfImport*` y `pdfPageCount`.
- [ ] **Step 3: commit:** `feat: importar PDF, imagen o una foto como cuaderno nuevo`

### Tarea 7: las portadas en la biblioteca de escritorio

**Files:**
- Modify: `crates/rnote-ui/src/library/card.rs:169-193` (`notebook_card`)
- Test: módulo `#[cfg(test)]` en `card.rs`, como `tabs.rs` y `location.rs`

**Interfaces:**
- Consumes: `Notebook.cover` (tarea 1).
- Produces:
  - `fn cover_css(argb: u32) -> String`: la regla CSS con el color de la tapa, por ejemplo
    `background-color: rgba(61, 111, 182, 1)` para 0xFF3D6FB6.
  - Con `cover: Some(_)`, `notebook_card` muestra una tapa del color con el nombre en una
    etiqueta en vez de la `Picture` con la miniatura. Sin portada queda como hoy.

- [ ] **Step 1: test que falla:** `cover_css_uses_the_argb_color`, que verifica 0xFF3D6FB6 y
  0x803F8F6B (alfa 0,5).
- [ ] **Step 2:** `cargo test -p rnote-ui library::card` → FALLA al compilar.
- [ ] **Step 3:** implementar. El CSS va en un `gtk4::CssProvider` por tarjeta, en el
  `Overlay` de `cover()`.
- [ ] **Step 4:** `cargo test -p rnote-ui library::` y `cargo clippy -p rnote-ui -- -D
  warnings` → en verde (las advertencias de upstream que ya estaban no cuentan; se compara
  contra el commit base). No se abre la app.
- [ ] **Step 5: commit:** `feat: las tarjetas de la biblioteca de escritorio muestran la portada`

### Tarea 8: prueba en la tablet

Con el usuario, y la app en primer plano chequeada en un comando aparte antes de instalar o
lanzar. Los resultados van al ledger.

1. "+" → Cuaderno nuevo con:
   - Título "Prueba portada", carpeta Álgebra, portada Verde.
   - Carta horizontal, Continua, Crema, Renglones de 8 mm.

   Crear abre el editor con esa hoja. En la biblioteca, la tarjeta es verde con el nombre.
2. "+" → Nota rápida en Álgebra: arranca igual que el punto 1, sin pantalla. Nota infinita:
   igual, con páginas infinitas.
3. "⋮" → Portada… → Rojo y después Sin portada: la tarjeta cambia.
4. Importar un PDF real de varios cientos de páginas, como Vector:
   - El progreso avanza.
   - Abre, se desplaza y se escribe sin trabarse.
   - La goma no borra el PDF.
   - Sincronizar lo sube a Drive: el archivo aparece en la carpeta `Rnote` con su tamaño.
   - Cancelar a la mitad de otra importación no deja ningún cuaderno.
5. Un PDF con contraseña: la pide, y con una equivocada la vuelve a pedir.
6. Importar imagen, y Sacar foto con la tablet vertical: la foto queda derecha.
7. Sincronizar y mirar `library.json` en Drive (desde la tablet, o con `run-as` sobre el
   archivo local): están `notebook_covers` y las claves de antes.

## Self-review (hecho)

- **Cobertura del spec:**
  - Objetivo 1 → tareas 5 y 6.
  - Objetivo 2 → tarea 5.
  - Objetivo 3 → tareas 1, 5 y 7.
  - Objetivo 4 → tareas 3 y 5.
  - Objetivo 5 → tareas 4 y 6.
  - Compatibilidad hacia adelante → tarea 1.
  - Subida grande → tarea 2.
  - Prueba con un PDF real → tarea 8.
- **Decisiones del plan que no están en el spec** (van como Ruling al ledger cuando se
  ejecuten):
  - La última configuración por carpeta vive en un JSON del dispositivo armado en Rust
    (`filesDir/notebook_defaults.json`, no sincronizado) y no en `SharedPreferences`, para
    poder testearla.
  - La configuración recordada incluye la portada y no las etiquetas.
  - "Nuevo" del editor pasa a ser Nota rápida.
  - Las fotos y las imágenes se achican a 2400 px en el lado mayor.
  - El PDF se importa una página por llamada (cada página llena el ancho), a cambio de
    reabrir el PDF en cada página.
