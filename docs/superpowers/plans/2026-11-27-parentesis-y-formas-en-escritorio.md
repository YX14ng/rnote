# Paréntesis grandes y formas en el escritorio: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reconocer paréntesis grandes como arcos simétricos y llevar el gesto de "lápiz quieto → forma" al escritorio, con un solo reconocedor compartido.

**Architecture:** El reconocedor (`shape_recognition`) y el seguimiento del trazo pasan de `rnote-android` a `rnote-engine`. El motor gana una operación que reemplaza el trazo en curso por la forma en un paso de deshacer, con el color y grosor del pincel. Android la llama como hoy; el lienzo GTK suma un temporizador de "quieto 1 s" que la llama también.

**Tech Stack:** Rust (rnote-engine, rnote-compose, parry2d-f64), GTK4/libadwaita (rnote-ui), Kotlin solo sin cambios de lógica.

**Spec:** `docs/superpowers/specs/2026-11-27-parentesis-y-formas-en-escritorio-design.md`

## Global Constraints

- Panza del paréntesis entre 6 % y 35 % de la cuerda; todos los puntos del mismo lado de la cuerda; error medio ≤ 5 % del tamaño (`MAX_FIT_ERROR`).
- El chequeo de arco va antes que el de línea; un trazo con esquina marcada nunca es arco.
- Enderezar la cuerda si está a menos de 6° (`SNAP_ANGLE`) de la vertical u horizontal.
- Gesto: 1000 ms quieto dentro de 6 px de pantalla; tamaño mínimo 24 px de pantalla / zoom.
- Forma reconocida: color y grosor configurados del pincel; rough/relleno de la herramienta de formas; formas abiertas (línea, polilínea, flecha, Bézier) sin relleno nunca.
- Interruptor "Reconocer formas" en Configuración del escritorio, prendido por defecto; textos en es, zh_Hans, zh_Hant, ja, ko, ru.
- Comportamiento de Android sin cambios salvo los paréntesis.
- Estilo de Rust de Google en código nuevo; no reformatear archivos de upstream; archivos ≤ 800 líneas (200–400 típico).
- Verificación verde antes de cada commit: `cargo test`, `cargo clippy --all-targets -- -D warnings` (host y `--target aarch64-linux-android` para rnote-android), Gradle lint/tests, `msgfmt -c`. Commits en español, un tipo por commit.

## Review Focus

1. Una línea recta a mano con un leve arco (2–4 %) debe seguir saliendo línea, no paréntesis → test en Task 4.
2. Un `<` obtuso (vértice ~120°) debe seguir saliendo polilínea → test en Task 4.
3. `(` y `)` dibujados de arriba abajo o de abajo arriba dan el mismo arco → test en Task 4.
4. En el escritorio, después de convertir, seguir moviendo y levantar el lápiz no agrega nada, y un solo deshacer deja la hoja vacía → test del rastreador en Task 2 (lógica compartida) y prueba con lápiz virtual en Task 5.
5. Si la herramienta de formas tiene otro color o relleno activo, la forma reconocida sale con el color del pincel y un paréntesis no se rellena → test en Task 3.

---

### Task 1: Mover el reconocedor a rnote-engine

**Files:**
- Move: `crates/rnote-android/src/shape_recognition/{mod,geometry,polygons,tests}.rs` → `crates/rnote-engine/src/shape_recognition/`
- Modify: `crates/rnote-engine/src/lib.rs` (`pub mod shape_recognition;`), `crates/rnote-android/src/lib.rs` (quitar el módulo), `crates/rnote-android/src/rnote_document/recognition.rs` (usar `rnote_engine::shape_recognition`)

**Interfaces:**
- Produces: `pub fn rnote_engine::shape_recognition::recognize(points: &[Vector2], min_size: f64) -> Option<Shape>` (antes `pub(crate)`).

- [ ] Mover con `git mv`, ajustar visibilidad e imports. Si rustfmt de rnote-engine formatea distinto que el de rnote-android, formatear solo estos archivos nuevos.
- [ ] `cargo test -p rnote-engine shape_recognition` → los mismos tests que había en Android pasan; `cargo test -p rnote-android` pasa.
- [ ] clippy host + aarch64 verde. Commit `refactor: el reconocedor de formas pasa al motor para compartirlo con el escritorio`.

### Task 2: Seguimiento del trazo y reemplazo en el motor

**Files:**
- Create: `crates/rnote-engine/src/shape_recognition/tracker.rs` (+ tests en `shape_recognition/tracker_tests.rs` si pasa de 400 líneas)
- Modify: `crates/rnote-android/src/rnote_document/recognition.rs` (delegar en el rastreador), sus tests siguen pasando sin cambios.

**Interfaces:**
- Produces:
  - `pub struct StrokeTracker` con `Default`.
  - `pub fn follow(&mut self, drawing: bool, ends: bool, points: impl IntoIterator<Item = Vector2>) -> bool`: `drawing` = muestras de pincel apoyado; `ends` = el trazo terminó o se canceló; devuelve `false` si las muestras son de un trazo ya convertido y no deben llegar al motor (y al terminar se rearma).
  - `pub fn reset(&mut self)`.
  - `pub fn snap(&mut self, engine: &mut Engine, min_size: f64, now: Instant) -> Option<WidgetFlags>`: si los puntos parecen una forma, termina el trazo con `PenEvent::Up` en el último punto, deshace, inserta la forma con `Engine::insert_recognized_shape` (Task 3; en este task, el `insert_shapes` actual) y marca el trazo como convertido; `None` si no hay forma o ya se convirtió.

- [ ] Tests en rnote-engine, con un `Engine` y eventos de pincel reales: `holding_turns_a_circle_into_one_shape_in_one_undo_step`, `samples_after_snapping_are_dropped_until_the_pen_lifts`, `no_shape_leaves_the_stroke_going`, `small_strokes_are_left_alone`. Correrlos, ver que fallan por falta de `StrokeTracker`.
- [ ] Implementar; Android delega (`Recognition` conserva `enabled` y `min_size_px` y contiene un `StrokeTracker`).
- [ ] Tests de rnote-engine y rnote-android verdes, clippy host + aarch64. Commit `refactor: el seguimiento del trazo para reconocer formas vive en el motor`.

### Task 3: Estilo de la forma reconocida

**Files:**
- Modify: `crates/rnote-engine/src/engine/import.rs` (reemplazar `insert_shapes` por `insert_recognized_shape`; solo lo usa el reconocimiento)

**Interfaces:**
- Produces: `pub fn Engine::insert_recognized_shape(&mut self, shape: Shape) -> WidgetFlags`: estilo de la herramienta de formas (liso o rough, relleno) con `stroke_color` y `stroke_width` del pincel en su estilo actual (`BrushConfig::style_for_current_options`); relleno `None` si la forma es `Line`, `Polyline`, `Arrow`, `QuadraticBezier` o `CubicBezier`; un paso de historial, sin selección.

- [ ] Test `recognized_shapes_take_the_brush_color_and_width`: herramienta de formas en rojo, grosor 9 y relleno; pincel azul grosor 3 → un rectángulo insertado tiene trazo azul, grosor 3 y relleno; una Bézier cuadrática tiene relleno `None`. Ver que falla.
- [ ] Implementar; `StrokeTracker::snap` pasa a usarla.
- [ ] Verde (engine + android). Commit `feat: las formas reconocidas toman el color y el grosor del pincel, y las abiertas no se rellenan`.

### Task 4: Paréntesis

**Files:**
- Create: `crates/rnote-engine/src/shape_recognition/arcs.rs`
- Modify: `crates/rnote-engine/src/shape_recognition/mod.rs` (orden de chequeos), `tests.rs` (tests nuevos); doc del módulo menciona paréntesis.
- Modify: textos de ayuda de Android que enumeran formas (`shape_recognition_hint` en los 7 `strings.xml`), si enumeran formas.

**Interfaces:**
- Produces: `pub(super) fn arcs::bracket(points: &[Vector2], size: f64) -> Option<QuadraticBezier>`: `None` si hay una esquina (giro > ~35° en un tramo corto, a criterio del implementador), si algún punto cae del otro lado de la cuerda más allá de una tolerancia chica, si la panza máxima está fuera de [0.06, 0.35] × cuerda, o si el error medio a la curva / `size` > `MAX_FIT_ERROR`. La curva: `start`/`end` en las puntas (cuerda enderezada si está a < 6° de un eje, girando alrededor de su punto medio), `cp` en la perpendicular por el medio de la cuerda, del lado de la panza, a 2 × panza.

- [ ] Tests (con el helper `freehand` existente): `recognizes_a_tall_parenthesis_as_a_symmetric_arc` (las puntas quedan con igual x, `cp.y` en el medio, `cp.x` del lado de la panza, para `(` y para `)`), `a_parenthesis_drawn_upwards_gives_the_same_arc`, `a_slightly_bent_line_stays_a_line` (panza 3 %), `an_obtuse_angle_bracket_stays_a_polyline` (vértice 120°), `a_u_stays_alone` (el `leaves_open_curves_alone` actual sigue), `a_small_parenthesis_is_left_alone` (bajo `min_size`). Ver que fallan los nuevos.
- [ ] Implementar `arcs.rs` y llamarlo para trazos abiertos antes del chequeo de línea; `recognize` devuelve `Shape::QuadraticBezier`.
- [ ] Verde (engine + android), clippy host + aarch64, Gradle si cambiaron strings. Commit `feat: reconocer paréntesis grandes como arcos simétricos`.

### Task 5: Gesto en el escritorio

**Files:**
- Create: `crates/rnote-ui/src/canvas/shape_hold.rs` (temporizador, ancla, `StrokeTracker`, encendido)
- Modify: `crates/rnote-ui/src/canvas/input.rs` (antes de mandar cada evento al motor: `follow`, y descartar si devuelve `false`; reiniciar ancla/temporizador al moverse > 6 px de pantalla), `crates/rnote-ui/src/canvas/mod.rs` (campo y propiedad `shape-recognition`), `crates/rnote-ui/data/app.gschema.xml.in` (clave `shape-recognition`, `b`, default `true`), `crates/rnote-ui/data/ui/settingspanel.ui` + `crates/rnote-ui/src/settingspanel/mod.rs` (`AdwSwitchRow` `general_shape_recognition_row`, título "Recognize Shapes", subtítulo "Hold the pen still at the end of a stroke to turn it into a shape"), `crates/rnote-ui/src/appwindow/appsettings.rs` (bind), `.po` de es/zh_Hans/zh_Hant/ja/ko/ru, `docs/fork-desktop.md`.

**Interfaces:**
- Consumes: `StrokeTracker` (Task 2), `Engine::current_pen_style_w_override()`, `Engine::camera.total_zoom()`.
- Behavior: solo con `PenStyle::Brush` y pincel apoyado; a los 1000 ms quieto llama `snap(engine, 24.0 / zoom, now)`, y si hubo forma aplica los `WidgetFlags` con `appwindow.handle_widget_flags`.

- [ ] Test unitario de la parte pura (p. ej. `ShapeHold::moved_enough(anchor, point, zoom) -> bool` con 6 px de pantalla) que falle primero.
- [ ] Implementar, compilar con meson (`meson setup --reconfigure` por el archivo nuevo), traducir con `addpo.py`, `msgfmt -c` en los 46 `.po`.
- [ ] Prueba en el binario devel con lápiz virtual: dibujar un `(` alto y quedarse quieto → arco; un cuadrado → rectángulo; levantar sin pausa → trazo a mano; apagar el interruptor → nada; un deshacer después de convertir deja la hoja como antes. Capturas con `niri msg action screenshot-window`.
- [ ] Commits separados: `feat: en el escritorio, dejar el lápiz quieto convierte el trazo en forma, como en Android` y `docs: el gesto de formas en el escritorio`.

### Cierre

- Verificación completa, push a origin, APK con el cambio de paréntesis (pedir ok antes de instalar), Flatpak con Rnote cerrado (pedir ok), memoria del proyecto.
