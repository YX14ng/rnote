# Etapa 6: búsqueda en lo escrito a mano (Android) — plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** dentro del cuaderno abierto en Android, un buscador que encuentra lo escrito a
mano y lo tipeado, sin distinguir mayúsculas ni acentos, marca las coincidencias sobre la
hoja y salta entre ellas ("2 de 7"), reconociendo la escritura en el idioma del cuaderno.

**Architecture:** Rust (`rnote-android`, testeable en el host) agrupa los trazos de lápiz en
renglones y fragmentos, sintetiza los tiempos que pide el reconocedor, guarda el índice por
cuaderno en la caché y busca (normalización, frases que cruzan fragmentos, texto tipeado
con posición exacta). Kotlin solo reconoce con ML Kit Digital Ink (descarga del modelo,
cola de fragmentos pendientes), dibuja la barra de búsqueda y las marcas.

**Tech Stack:** Rust 2024 (`rnote-engine`, `serde_json`, `sha2`, `unicode-normalization`),
Kotlin sin AndroidX propio (Activities de `android.app`), `com.google.mlkit:digital-ink-recognition`.

**Spec:** `docs/superpowers/specs/2026-11-10-biblioteca-sync-busqueda-design.md`, sección
"Búsqueda (solo Android)". Pedido posterior del usuario (2026-11-12): "el idioma agregalo
en la búsqueda".

## Global Constraints

- Solo Android; sin búsqueda en la PC ni global desde la biblioteca (spec, no objetivos).
- El índice es local, en la caché de la app, y no se sincroniza.
- Reconocedor: ML Kit Digital Ink, modelo descargado una vez; mientras no esté, el buscador
  lo avisa y busca solo el texto tipeado.
- Índice por cuaderno: por fragmento, el hash de sus trazos, su rectángulo y los 3 mejores
  candidatos. Se actualiza en segundo plano, solo lo que cambió, al abrir el cuaderno y unos
  segundos después de dejar de escribir.
- Búsqueda sin distinguir mayúsculas ni acentos, por subcadena, sobre los candidatos y el
  texto tipeado. Resultados resaltados sobre la hoja, anterior/siguiente con "n de N", la
  vista salta a cada uno. El ícono de buscar va en la fila 1 de la barra y abre un campo ahí.
- Fórmulas: se indexa lo que el reconocedor devuelva, sin tratamiento especial.
- Textos de UI en `values/strings.xml` (inglés) y `values-es/strings.xml` (español).
- Rust nuevo con Google Rust Style Guide y los `[lints]` del crate; upstream no se toca.
- Tablet: chequear `topResumedActivity` en un comando aparte antes de cada instalación o
  lanzamiento; no instalar ni lanzar si el usuario está en otra app o en Rnote.
- Verificación: `cargo test/clippy --all-targets -D warnings/fmt --check -p rnote-android`,
  clippy aarch64, `./gradlew lintDebug testDebugUnitTest` (base: 0 errores, 5 avisos).

## Decisiones del asistente (revisables)

- **Idioma:** cada cuaderno tiene el suyo, elegido en la barra de búsqueda con un chip
  ("ES"): Español (`es`), English (`en-US`), 中文 (`zh-Hani-CN`). Se guarda en el índice
  local del cuaderno; uno nuevo hereda el último elegido (preferencia de la app, por defecto
  `es`). Cambiarlo vacía el índice y reconoce todo de nuevo. No se sincroniza, como el índice.
- **Unidad de reconocimiento:** el fragmento (trazos de un renglón separados por un hueco
  grande, ≈ una palabra). Una búsqueda de varias palabras se compara contra el renglón
  armado con el mejor candidato de cada fragmento, y marca la unión de los fragmentos que
  cubre.
- **Umbrales:** renglón = trazos cuyo centro vertical cae dentro de la banda del renglón
  (alto de banda = 1,0 × mediana de altos de trazo del documento); fragmento nuevo cuando el
  hueco horizontal supera 0,6 × esa mediana. Tiempos sintéticos: 10 ms entre puntos, 100 ms
  entre trazos. Indexado 3 s después de la última edición.

## Review Focus

1. Cuaderno con cientos de fragmentos (un PDF anotado, un cuaderno de un cuatrimestre):
   abrirlo no traba la UI y el indexado avanza en segundo plano y se corta al cerrarlo.
   → Tarea 5, test `indexer_runs_one_fragment_at_a_time_and_stops_on_close`.
2. Sin conexión la primera vez (modelo no descargado): buscar encuentra lo tipeado y avisa;
   cuando vuelve la red, el modelo baja y el índice se arma solo.
   → Tarea 4 (`ModelState`), Tarea 6 (texto del aviso).
3. Borrar o mover trazos: las entradas viejas se van del índice y no aparecen resultados
   fantasma. → Tarea 2, test `pending_drops_fragments_that_are_gone`.
4. Búsqueda con acentos o mayúsculas distintas ("ALGEBRA" encuentra "álgebra"; "funcion"
   encuentra "función") y con varias palabras ("matriz inversa" que cruza dos fragmentos).
   → Tarea 2, tests `search_ignores_case_and_accents`, `phrase_spans_fragments_of_a_line`.
5. Índice de otra versión del cuaderno (la sincronización bajó otra): los hashes no
   coinciden, se reconoce lo nuevo y nunca se muestra un rectángulo de la versión vieja.
   → cubierto por el keying por hash (Tarea 2, `unchanged_fragments_keep_their_candidates`).

---

### Tarea 1: fragmentos de escritura en Rust

**Files:**
- Create: `crates/rnote-android/src/ink_index/mod.rs`, `ink_index/fragments.rs`, `ink_index/tests/fragments.rs`
- Modify: `crates/rnote-android/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub(crate) struct InkStroke { pub points: Vec<(f64, f64)> }` (de cada `Stroke::BrushStroke`, en orden de creación del store)
  - `pub(crate) struct Fragment { pub hash: String, pub rect: [f64; 4] /* x0, y0, x1, y1 */, pub line: usize, pub strokes: Vec<Vec<(f64, f64, i64)>> /* x, y, t ms */ }`
  - `pub(crate) fn fragments(strokes: &[InkStroke]) -> Vec<Fragment>` — ordenados por renglón y luego por x; `hash` = sha256 hex de los puntos redondeados a 0,01.
  - `pub(crate) fn ink_strokes(engine: &Engine) -> Vec<InkStroke>`

- [ ] **Step 1: tests:** `two_words_on_a_line_are_two_fragments`; `two_lines_are_two_lines`
  (trazos a y = 0 y y = 3 × alto → `line` 0 y 1); `a_dotted_i_stays_in_its_word` (el punto
  de una i, chico y arriba, cae en el fragmento de su palabra); `times_are_synthetic_and_increasing`
  (10 ms entre puntos, +100 ms entre trazos, empieza en 0); `hash_ignores_jitter_below_a_hundredth`;
  `no_strokes_no_fragments`.
- [ ] **Step 2:** `cargo test -p rnote-android ink_index` → no compila.
- [ ] **Step 3:** implementar con los umbrales de "Decisiones".
- [ ] **Step 4:** tests verdes; clippy host y aarch64; fmt.
- [ ] **Step 5:** commit `feat: agrupar la escritura en renglones y fragmentos para reconocerla`

### Tarea 2: índice, normalización y búsqueda en Rust

**Files:**
- Create: `ink_index/store.rs`, `ink_index/search.rs`, `ink_index/tests/{store,search}.rs`
- Modify: `crates/rnote-android/Cargo.toml` (`unicode-normalization` en el workspace)

**Interfaces:**
- Consumes: Tarea 1.
- Produces:
  - `pub(crate) struct InkIndex { pub language: String, pub entries: BTreeMap<String /* hash */, Entry> }`, `Entry { rect: [f64; 4], candidates: Vec<String> }`; `load(path) -> InkIndex` (archivo faltante o ilegible → vacío con el idioma dado), `save(path)` atómico.
  - `pub(crate) fn pending(index: &mut InkIndex, fragments: &[Fragment]) -> Vec<Fragment>` — quita del índice los hashes que ya no están y devuelve los fragmentos sin entrada.
  - `pub(crate) fn normalize(text: &str) -> String` — NFD, sin marcas combinantes, minúsculas, espacios colapsados.
  - `pub(crate) struct Match { pub rects: Vec<[f64; 4]> }`
  - `pub(crate) fn search(index: &InkIndex, fragments: &[Fragment], typed: &[TypedText], query: &str) -> Vec<Match>` — ordenados por posición (y y luego x del primer rectángulo); query vacía → nada.
  - `pub(crate) struct TypedText { pub text: String, pub rects_of: Box<dyn Fn(usize, usize) -> Vec<[f64; 4]>> }` y `typed_texts(engine) -> Vec<TypedText>` con `TextStroke::get_selection_rects_for_cursors` para el rango de bytes coincidente.

- [ ] **Step 1: tests:** `search_ignores_case_and_accents` ("ALGEBRA"→"álgebra", "funcion"→"Función");
  `any_of_three_candidates_matches`; `phrase_spans_fragments_of_a_line` (rect = unión de los dos);
  `phrase_never_spans_lines`; `typed_text_matches_with_its_own_rects`;
  `pending_drops_fragments_that_are_gone`; `unchanged_fragments_keep_their_candidates`;
  `changing_the_language_empties_the_index`; `unreadable_index_file_is_an_empty_index`;
  `results_are_in_reading_order`.
- [ ] **Step 2:** → no compila.
- [ ] **Step 3:** implementar. Normalizar índice y query de la misma forma; subcadena.
- [ ] **Step 4–5:** verde, clippy, fmt; commit `feat: índice de escritura reconocida y búsqueda sin acentos ni mayúsculas`

### Tarea 3: puente JNI

**Files:**
- Create: `crates/rnote-android/src/jni_bridge/ink.rs`
- Modify: `jni_bridge/mod.rs`, `android/viewer/.../NativeBridge.kt`, `NativeDocument.kt`

**Interfaces:**
- Produces (JNI, JSON en strings como el resto del puente):
  - `inkPending(handle, indexPath, language): String` — `[{"hash","rect":[4],"strokes":[[[x,y,t],...],...]}]`; guarda el índice podado.
  - `inkAdd(indexPath, language, hash, rectJson, candidatesJson)` — agrega una entrada.
  - `inkSearch(handle, indexPath, query): String` — `[{"rects":[[4],...]}]`.
  - `inkLanguage(indexPath, fallback): String`.
- [ ] **Step 1:** test Rust de las funciones puras que el puente traduce (`pending_json_round_trips`, `search_json_shape`).
- [ ] **Step 2–4:** RED, implementar, verde + clippy aarch64.
- [ ] **Step 5:** commit `feat: puente JNI del índice de escritura`

### Tarea 4: reconocedor ML Kit

**Files:**
- Create: `android/viewer/app/src/main/kotlin/.../InkRecognizer.kt`, `app/src/test/.../InkLanguageTest.kt`
- Modify: `app/build.gradle.kts` (`implementation("com.google.mlkit:digital-ink-recognition:<última estable>")`)

**Interfaces:**
- Produces: `enum class InkLanguage(val tag: String, val label: Int, val chip: String) { ES("es"), EN("en-US"), ZH("zh-Hani-CN") }` con `fromTag`;
  `sealed interface ModelState { Ready; Downloading; Offline; Failed(message) }`;
  `class InkRecognizer(context)`: `ensureModel(language, onState)`, `recognize(fragment, language, done: (Result<List<String>>) -> Unit)` (3 mejores candidatos, `WritingArea` del tamaño del rectángulo), `close()`.
- [ ] **Step 1:** test Kotlin `InkLanguageTest` (tags, `fromTag` desconocido → ES).
- [ ] **Step 2–4:** RED, implementar, verde; `lintDebug` sin avisos nuevos.
- [ ] **Step 5:** commit `feat: reconocer escritura con ML Kit en el idioma del cuaderno`

### Tarea 5: indexado en segundo plano

**Files:**
- Create: `.../InkIndexer.kt`, `.../InkSchedule.kt` (lógica pura), `app/src/test/.../InkScheduleTest.kt`
- Modify: `MainActivity.kt` (crear al abrir, `onEdited()` tras cada edición, `close()` al cerrar)

**Interfaces:**
- Produces: `InkSchedule` (puro: `edited(now)`, `due(now): Boolean` a los 3 s; un fragmento a la vez; `closed` corta); `InkIndexer(recognizer, renderer, indexFile)` con `start()`, `onEdited()`, `close()`, `language`, `setLanguage(l)`.
- [ ] **Step 1:** tests `indexing_waits_three_seconds_after_the_last_edit`, `indexer_runs_one_fragment_at_a_time_and_stops_on_close`.
- [ ] **Step 2–4:** RED, implementar, verde, lint.
- [ ] **Step 5:** commit `feat: indexar la escritura en segundo plano al abrir y al dejar de escribir`

### Tarea 6: barra de búsqueda y marcas

**Files:**
- Create: `.../SearchBar.kt`, `res/drawable/ic_search.xml`, `ic_chevron_up.xml` (si falta)
- Modify: `EditorTopBar.kt` (ícono en la fila 1), `NoteCanvasView.kt` (dibujar rectángulos: todos con relleno suave, el actual más fuerte; `showMatch(rects)` centra la vista), `MainActivity.kt`, `values/strings.xml`, `values-es/strings.xml`

**Interfaces:**
- Produces: `SearchBar` con campo, chip de idioma (menú de los tres), "n de N", anterior/siguiente, cerrar (borra las marcas); busca 300 ms después de dejar de tipear; avisos: "Descargando el reconocedor de <idioma>…", "Sin conexión: por ahora solo se busca texto tipeado", "Reconociendo la escritura: n fragmentos pendientes".
- [ ] **Step 1:** test Kotlin puro `match_counter_wraps` (`MatchCursor`: siguiente tras el último vuelve al primero; "0 de 0" sin resultados).
- [ ] **Step 2–4:** RED, implementar, verde, lint (0 errores, avisos de base).
- [ ] **Step 5:** commit `feat: buscador en el cuaderno con marcas sobre la hoja y salto entre resultados`

### Tarea 7: prueba en la tablet (con el usuario)

- [ ] Escribir "álgebra lineal" a mano y "Matriz" tipeado; buscar "algebra", "LINEAL",
  "algebra lineal", "matriz" → marcas correctas y "n de N"; anterior/siguiente salta.
- [ ] Cambiar el idioma a English y volver: reconoce de nuevo.
- [ ] Borrar una palabra: deja de aparecer. Abrir un cuaderno largo: la UI no se traba.
- [ ] Modo avión con el modelo de otro idioma sin bajar: aviso y búsqueda de tipeado.
- [ ] Ledger, push al cerrar.
