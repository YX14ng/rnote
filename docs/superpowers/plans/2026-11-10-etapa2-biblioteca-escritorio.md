# Etapa 2: biblioteca en el Rnote de escritorio y build propio — plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** El Rnote de escritorio abre en una pantalla de inicio con la biblioteca (la misma
de Android: carpetas, etiquetas, papelera, miniaturas, importar `.xopp`), con un botón
"Inicio" en el editor, y se instala como Flatpak propio al lado del Rnote oficial.

**Architecture:** La lógica que no dibuja va a `crates/rnote-library` (secciones, orden,
caché de miniaturas en disco, `.rnote` ya importado de un `.xopp`), con tests. En
`rnote-ui`, un módulo nuevo `library/` con la vista `RnLibraryView` (libadwaita), que la
ventana muestra en un `AdwViewStack` como alternativa al área de pestañas. El build propio
cambia solo el app id y el nombre visible por opciones de meson nuevas, y un manifiesto
Flatpak de release con red.

**Tech Stack:** Rust 2024 del workspace, gtk4-rs 0.11 (`v4_22`), libadwaita 0.9 (`v1_9`),
meson, Flatpak (GNOME 50 + `rust-stable`), `rnote-library` de la etapa 1.

**Spec:** `docs/superpowers/specs/2026-11-10-biblioteca-sync-busqueda-design.md` (secciones
"Biblioteca", "Pantalla de inicio", "Escritorio: cómo llega al usuario", "Orden propuesto"
etapa 2).

## Global Constraints

- Código nuevo de `rnote-library`: Google Rust Style Guide y su `rustfmt.toml`. Código nuevo
  de `rnote-ui`: la Google Rust Style Guide en nombres y documentación, formateado con el
  rustfmt del workspace (el de upstream), que no debe cambiar archivos upstream:
  `cargo fmt -p rnote-ui -- --check` sin diferencias fuera de los archivos nuevos o tocados.
- Archivos de 200–400 líneas típico, 800 tope.
- No romper la apertura de archivos sueltos ("Abrir", arrastrar, abrir desde el gestor de
  archivos), ni pestañas, ni el explorador de espacios de trabajo existente.
- Biblioteca: raíz configurable en la clave gsettings `library-dir` (string, `""` =
  `~/Documents/Rnote`, con la carpeta de documentos de XDG). En la PC del usuario,
  `/mnt/Storage/HandNotes`.
- App id del build propio: `io.github.yx14ng.rnote`; nombre visible `Rnote (YX14ng)`. El
  path de recursos (`/com/github/flxzt/rnote/`) no cambia. El oficial
  (`com.github.flxzt.rnote` 0.14.2) queda intacto.
- Textos nuevos en inglés con `gettext`/`translatable="yes"`, archivos nuevos en
  `crates/rnote-ui/po/POTFILES`, y traducciones en `crates/rnote-ui/po/es.po` y
  `crates/rnote-ui/po/zh_CN.po` (el escritorio del usuario está en `zh_CN`).
- Archivos `.rs` nuevos de `rnote-ui` en `rnote_ui_sources` de `crates/rnote-ui/src/meson.build`;
  `.ui` nuevos en `crates/rnote-ui/data/resources.gresource.xml`.
- Ninguna credencial en el repo.
- Commits en español `tipo: descripción`, uno por tipo; verificación en verde antes de cada
  commit: `cargo test -p rnote-library`, `cargo test -p rnote-ui`,
  `cargo clippy -p rnote-library -p rnote-ui --all-targets -- -D warnings` (si upstream ya
  tiene avisos en `rnote-ui`, la línea base se anota en el ledger y no se suman avisos
  nuevos), `meson compile -C _mesonbuild`. Push al cerrar la etapa.
- La PC es del usuario: antes de abrir una ventana de Rnote para probar, avisarle; nunca
  cerrar ni tocar otras ventanas suyas. Capturas con
  `niri msg action screenshot-window --id <id> --write-to-disk true`.

## Review Focus

1. Cuaderno abierto en una pestaña al que se le renombra, mueve o renombra la carpeta desde
   la biblioteca: la pestaña sigue guardando en la ruta nueva, no en la vieja ni en ninguna.
2. Cuaderno abierto en una pestaña que se manda a la papelera (o su carpeta se borra): se
   rechaza con un aviso, no se mueve un archivo que la pestaña volvería a escribir.
3. Tocar dos veces la tarjeta de un `.xopp` ya importado: abre el `.rnote` existente o
   pregunta, nunca crea "X (2).rnote" en silencio.
4. Raíz de la biblioteca inexistente, sin permiso o en un disco desmontado: la vista
   muestra el error y el editor sigue andando; no hay pánico.
5. Abrir un cuaderno desde la biblioteca con una pestaña vacía: la reutiliza; con el
   cuaderno ya abierto: enfoca esa pestaña, no abre otra.

---

### Task 1: secciones, orden y caché de miniaturas en `rnote-library`

**Files:**
- Create: `crates/rnote-library/src/section.rs`, `crates/rnote-library/src/thumbnail_cache.rs`
- Modify: `crates/rnote-library/src/lib.rs` (módulos y `pub use`), `crates/rnote-library/src/import.rs`
- Test: `crates/rnote-library/tests/section.rs`, `crates/rnote-library/tests/thumbnail_cache.rs`,
  `crates/rnote-library/tests/ops.rs` (caso de importado)

**Interfaces:**
- Consumes: `Library`, `Notebook`, `NotebookKind`, `TrashEntry`, `render_thumbnail`,
  `thumbnail_key` (etapa 1).
- Produces:
  - `pub enum Section { All, Uncategorized, Folder(String), Tag(String), Trash }`
    (`Debug, Clone, PartialEq, Eq`).
  - `pub enum SortOrder { Modified, Name }` (`Debug, Clone, Copy, PartialEq, Eq, Default` = `Modified`).
  - `pub fn filter(notebooks: &[Notebook], section: &Section) -> Vec<Notebook>` (para
    `Trash` devuelve vacío: la papelera se lista con `trash_entries`).
  - `pub fn sort(notebooks: &mut [Notebook], order: SortOrder)`: `Modified` = más nuevo
    primero; `Name` = sin distinguir mayúsculas, empate por más nuevo. Igual que Android.
  - `pub struct SectionCounts { pub all: usize, pub uncategorized: usize, pub folders: BTreeMap<String, usize>, pub tags: BTreeMap<String, usize>, pub trash: usize }`
    y `pub fn counts(notebooks: &[Notebook], folders: &[String], trash: usize) -> SectionCounts`
    (toda carpeta de `folders` aparece, aunque tenga 0).
  - `pub struct ThumbnailCache` con `pub fn new(dir: PathBuf) -> Self` y
    `pub fn get_or_render(&self, root: &Path, notebook: &Notebook, size_px: u32) -> anyhow::Result<Vec<u8>>`:
    lee `<dir>/<thumbnail_key>-<size_px>.png` si existe; si no, renderiza con
    `render_thumbnail`, escribe atómico (tmp + rename) y devuelve los bytes. Error para
    `NotebookKind::Xopp`.
  - `impl Library { pub fn imported_rnote_of(&self, xopp_path: &str) -> Option<String> }`:
    la ruta de `<misma carpeta>/<stem>.rnote` si existe (sin distinguir mayúsculas, vía
    `name_taken`), si no `None`.

- [ ] **Step 1: Escribir los tests que fallan**

`tests/section.rs` (cuadernos armados a mano, sin disco):
`filter_all_returns_every_notebook`, `filter_uncategorized_keeps_only_root_notebooks`,
`filter_folder_matches_the_folder_name_exactly`, `filter_tag_keeps_notebooks_with_that_tag`,
`filter_trash_is_empty`, `sort_by_modified_puts_the_newest_first`,
`sort_by_name_ignores_case_and_breaks_ties_by_newest` (con `"b"`, `"A"`, `"a"` → `A`/`a`
por fecha, luego `b`), `counts_include_empty_folders` (carpeta sin cuadernos → `0`),
`counts_count_a_notebook_once_per_tag`.

`tests/thumbnail_cache.rs` (con `tempfile`, un `.rnote` vacío hecho con
`Engine::default().save_as_rnote_bytes`): `renders_once_and_then_reads_the_cache`
(segundo llamado devuelve los mismos bytes y el `.png` no cambia de mtime),
`a_modified_notebook_gets_a_new_thumbnail` (cambiar el archivo → otra clave, otro `.png`),
`different_sizes_are_cached_apart`, `xopp_is_an_error`.

`tests/ops.rs`: `imported_rnote_of_finds_the_rnote_beside_the_xopp`,
`imported_rnote_of_is_none_before_importing`, `imported_rnote_of_ignores_case`.

- [ ] **Step 2: Correrlos y verlos fallar**

Run: `cargo test -p rnote-library`
Expected: FAIL de compilación: `Section`, `ThumbnailCache`, `imported_rnote_of` no existen.

- [ ] **Step 3: Implementar** `section.rs`, `thumbnail_cache.rs` y `imported_rnote_of`
con las firmas de arriba; `pub use section::{Section, SectionCounts, SortOrder, counts, filter, sort}`
y `pub use thumbnail_cache::ThumbnailCache` en `lib.rs`.

- [ ] **Step 4: Correr en verde**

Run: `cargo test -p rnote-library && cargo clippy -p rnote-library --all-targets -- -D warnings`
Expected: los 37 tests previos + los nuevos pasan; clippy limpio.

- [ ] **Step 5: Commit**

```bash
git add crates/rnote-library
git commit -m "feat: secciones, orden y caché de miniaturas en la biblioteca compartida"
```

### Task 2: app id y nombre propios por opciones de meson

**Files:**
- Modify: `meson_options.txt`, `meson.build` (después del bloque de `profile`),
  `crates/rnote-ui/data/meson.build` (nombre del `.desktop`)
- Create: `build-aux/dev-setup.sh` (meson setup del fork en `_mesonbuild`)

**Interfaces:**
- Produces: opciones `-Dapp-id=<id>` (string, `''` = el de siempre) y `-Dapp-name=<nombre>`
  (string, `''` = `Rnote`). `app-id` reemplaza `app_id` en ambos perfiles;
  `app-name` reemplaza `app_name_capitalized` (título de ventana, "Acerca de", `.desktop`,
  metainfo). `app_name` (`rnote`: binario, gettext, pkgdatadir) y `app_idpath` no cambian.
- `build-aux/dev-setup.sh`: `meson setup --prefix=/usr -Dprofile=devel -Dcli=false
  -Dapp-id=io.github.yx14ng.rnote.Devel -Dapp-name='Rnote (YX14ng)' _mesonbuild` (con
  `--reconfigure` si ya existe). Genera `crates/rnote-ui/src/config.rs`, que hace falta para
  `cargo test -p rnote-ui`.

- [ ] **Step 1: Verificar la falla**

Run: `meson setup --prefix=/usr -Dapp-id=io.github.yx14ng.rnote _probe`
Expected: FAIL "Unknown option: app-id".

- [ ] **Step 2: Implementar** las dos opciones (sin tocar el valor por defecto de upstream)
y `build-aux/dev-setup.sh`.

- [ ] **Step 3: Verificar los archivos generados**

Run: `rm -rf _probe && meson setup --prefix=/usr -Dcli=false -Dapp-id=io.github.yx14ng.rnote -Dapp-name='Rnote (YX14ng)' _probe && meson compile -C _probe && desktop-file-validate _probe/crates/rnote-ui/data/io.github.yx14ng.rnote.desktop && glib-compile-schemas --strict --dry-run _probe/crates/rnote-ui/data && grep -E '^Name=|^Icon=' _probe/crates/rnote-ui/data/io.github.yx14ng.rnote.desktop && grep -n 'APP_ID' crates/rnote-ui/src/config.rs`
Expected: valida; `Name=Rnote (YX14ng)`, `Icon=io.github.yx14ng.rnote`; `APP_ID = "io.github.yx14ng.rnote"`.
Después `rm -rf _probe && build-aux/dev-setup.sh` y `cargo test -p rnote-ui` en verde
(línea base de tests y de clippy de `rnote-ui` anotada en el ledger).

- [ ] **Step 4: Commit**

```bash
git add meson_options.txt meson.build crates/rnote-ui/data/meson.build build-aux/dev-setup.sh
git commit -m "chore: app id y nombre visibles configurables para instalar el fork al lado del oficial"
```

### Task 3: la ventana gana la pantalla de inicio

**Files:**
- Create: `crates/rnote-ui/src/library/mod.rs` (`RnLibraryView`, por ahora con su barra y un
  `AdwStatusPage` de error/vacío), `crates/rnote-ui/src/library/location.rs`,
  `crates/rnote-ui/data/ui/library/libraryview.ui`
- Modify: `crates/rnote-ui/data/ui/appwindow.ui` (`AdwViewStack` `main_stack` dentro del
  `AdwTabOverview`, páginas `library` y `editor`), `crates/rnote-ui/src/appwindow/imp.rs`
  y `mod.rs` (template child, `show_library()`, `show_editor()`), `appwindow/actions.rs`
  (`win.show-library`, `win.show-editor`), `crates/rnote-ui/data/ui/mainheader.ui` (botón
  `go-home-symbolic` "Library" al principio de `quickactions_box`),
  `crates/rnote-ui/src/app/mod.rs` (registrar el tipo; `new_appwindow_init_show(None)`
  muestra la biblioteca, con archivo muestra el editor), `crates/rnote-ui/data/app.gschema.xml.in`
  (`library-dir` s `""`, `library-sort-by-name` b `false`, `library-list-view` b `false`),
  `crates/rnote-ui/src/main.rs` (`mod library`), meson y gresource.
- Test: `#[cfg(test)]` en `library/location.rs`

**Interfaces:**
- Produces:
  - `pub(crate) fn library_root(setting: &str, documents: Option<PathBuf>, home: &Path) -> PathBuf`:
    `setting` no vacío → esa ruta; vacío → `documents/Rnote`, o `home/Documents/Rnote`.
  - `RnAppWindow::show_library(&self)` (refresca la vista y la muestra) y
    `RnAppWindow::show_editor(&self)`; `RnAppWindow::library_view(&self) -> RnLibraryView`.
  - `RnLibraryView::init(&self, appwindow: &RnAppWindow)` y `RnLibraryView::refresh(&self)`
    (abre `rnote_library::Library` en `library_root(...)`, creando la carpeta; si falla,
    muestra el error en la `AdwStatusPage`, con botón "Choose Folder…", y no entra en
    pánico).
  - En la barra de la biblioteca: botón "Open Tabs" (`win.show-editor`) y menú con
    "Library Folder…" (`gtk::FileDialog::select_folder` → guarda `library-dir` →
    `refresh`). Ruling del plan: el cambio de carpeta va en el menú de la biblioteca, no en
    el panel de ajustes, para que se vea donde se usa.
  - `open_file_w_dialogs` llama a `show_editor()` salvo para carpetas
    (`FileType::Folder`).

- [ ] **Step 1: Test que falla**: `library_root_uses_the_setting_when_set`,
  `library_root_defaults_to_documents_rnote`, `library_root_falls_back_to_home_documents`.

- [ ] **Step 2:** Run: `cargo test -p rnote-ui library_root` — Expected: FAIL, no existe.

- [ ] **Step 3: Implementar** lo de arriba.

- [ ] **Step 4: Verificar**

Run: `cargo test -p rnote-ui && cargo clippy -p rnote-ui --all-targets -- -D warnings && meson compile -C _mesonbuild`
Expected: verde. Prueba manual (avisando al usuario antes), con
`GSETTINGS_SCHEMA_DIR=_mesonbuild/crates/rnote-ui/data` y `library-dir` apuntando a una
carpeta temporal: abre en la biblioteca; "Open Tabs" muestra el editor; el botón de inicio
vuelve; `rnote <archivo>.rnote` abre directo en el editor; una raíz inexistente sin
permiso muestra el error.

- [ ] **Step 5: Commit** — `feat: pantalla de inicio en la ventana de escritorio con botón Inicio`

### Task 4: secciones, tarjetas y abrir cuadernos

**Files:**
- Create: `crates/rnote-ui/src/library/sidebar.rs` (lista de secciones con cantidades;
  carpetas y etiquetas con `+`), `crates/rnote-ui/src/library/card.rs` (tarjeta: miniatura
  `gtk::Picture`, nombre, fecha o "Xournal++ · import", botón de menú; modo lista),
  `crates/rnote-ui/src/library/thumbnails.rs` (carga en segundo plano con
  `gio::spawn_blocking` + `ThumbnailCache` en `glib::user_cache_dir()/rnote/thumbnails`,
  `gdk::Texture::from_bytes`), `crates/rnote-ui/src/library/open.rs`
- Modify: `library/mod.rs`, `libraryview.ui` (`AdwOverlaySplitView` con la columna y un
  `gtk::FlowBox` en un `gtk::ScrolledWindow`; botones de vista, orden y seleccionar)

**Interfaces:**
- Consumes: Task 1 (`Section`, `filter`, `sort`, `counts`, `ThumbnailCache`,
  `imported_rnote_of`), Task 3 (`show_editor`, `library_root`).
- Produces:
  - `RnLibraryView::section(&self) -> Section`, `set_section(&self, Section)`.
  - Tarjeta "New" primera (no en la papelera): `library.new_notebook_path(folder, &gettext("Untitled"))`,
    pestaña nueva con `new_canvas_wrapper` + `append_wrapper_new_tab`, guardada enseguida
    con `canvas.save_document_to_file(&gio::File::for_path(abs))`, y `show_editor()`.
    En "Sin categoría", "Todas" y etiquetas se crea en la raíz; en una carpeta, ahí.
  - `pub(crate) async fn open_notebook(appwindow: &RnAppWindow, root: &Path, notebook: &Notebook)`
    en `open.rs`: `.rnote` → `open_file_w_dialogs(file, None, true)` (que ya reutiliza la
    pestaña vacía y enfoca la abierta); `.xopp` → si `imported_rnote_of` da `Some`,
    `adw::AlertDialog` "Already imported" con "Open" (por defecto) e "Import Again";
    si no, `import_xopp` y abrir el resultado.
  - Orden y vista persistidos en `library-sort-by-name` y `library-list-view`.
  - Miniatura: tapa lisa mientras se genera; tamaño 2× el ancho de la tarjeta (190 px).

- [ ] **Step 1: Test que falla** en `open.rs`, con la decisión separada de GTK:
  `pub(crate) enum OpenPlan { Rnote(String), AskReimport { existing: String }, Import }` y
  `pub(crate) fn open_plan(library: &Library, notebook: &Notebook) -> OpenPlan`; tests
  `rnote_opens_directly`, `imported_xopp_asks_before_importing_again`, `new_xopp_imports`.

- [ ] **Step 2:** Run: `cargo test -p rnote-ui open_plan` — Expected: FAIL, no existe.

- [ ] **Step 3: Implementar** columna, tarjetas, miniaturas y apertura.

- [ ] **Step 4: Verificar**

Run: `cargo test -p rnote-ui && cargo clippy -p rnote-ui --all-targets -- -D warnings && meson compile -C _mesonbuild`
Expected: verde. Manual, sobre una COPIA de `/mnt/Storage/HandNotes` en el scratchpad
(nunca la original en esta tarea): aparecen "Algebra Lineal" (`.rnote` con miniatura) y los
dos `.xopp` como "Xournal++ · import", no los `.autosave`; abrir dos veces enfoca la misma
pestaña; tocar dos veces el `.xopp` pregunta; "New" crea "Untitled.rnote" y abre el editor;
la cantidad de cada sección coincide.

- [ ] **Step 5: Commit** — `feat: secciones, tarjetas con miniatura y apertura de cuadernos en la biblioteca de escritorio`

### Task 5: operaciones, papelera, carpetas, etiquetas y selección múltiple

**Files:**
- Create: `crates/rnote-ui/src/library/actions.rs` (menú de tarjeta y sus diálogos),
  `crates/rnote-ui/src/library/manage.rs` (carpetas y etiquetas: crear, renombrar, borrar,
  color con `gtk::ColorDialogButton`), `crates/rnote-ui/src/library/tabs.rs`
- Modify: `library/mod.rs`, `card.rs`, `sidebar.rs`, `libraryview.ui` (barra de selección:
  mover, etiquetar, a la papelera; en la papelera "Empty Trash")

**Interfaces:**
- Consumes: operaciones de `Library` de la etapa 1 (`rename`, `move_to`, `duplicate`,
  `trash`, `restore`, `delete_forever`, `empty_trash`, `create_folder`, `rename_folder`,
  `delete_folder`, `create_tag`, `update_tag`, `delete_tag`, `set_tags`); `get_all_tabs`,
  `canvas().output_file()/set_output_file()`.
- Produces (en `tabs.rs`, sin GTK en la lógica):
  - `pub(crate) fn retarget(path: &Path, from: &Path, to: &Path) -> Option<PathBuf>`: si
    `path == from` → `to`; si `path` está dentro de la carpeta `from` → la misma ruta bajo
    `to`; si no `None`. Compara componentes, no texto (`/a/b` no está dentro de `/a/bc`).
  - `RnAppWindow`-side: `retarget_open_tabs(appwindow, from, to)` aplica `retarget` a la
    salida de cada pestaña ANTES de que el watcher del archivo vea el movimiento;
    `open_tab_inside(appwindow, path) -> Option<String>` (título de la pestaña) para
    rechazar con un toast "Close “{name}” first" la papelera, borrar carpeta y borrar
    definitivamente de algo abierto.
  - Etiquetar varios: cambia solo las etiquetas tildadas o destildadas (como Android).
  - Borrar una carpeta no vacía: `adw::AlertDialog` de confirmación (destructivo).
  - Errores de la biblioteca: toast con el mensaje, y la vista se refresca.

- [ ] **Step 1: Tests que fallan**: `retarget_maps_the_file_itself`,
  `retarget_maps_files_inside_a_renamed_folder`, `retarget_ignores_a_sibling_with_a_common_prefix`,
  `retarget_ignores_unrelated_paths`.

- [ ] **Step 2:** Run: `cargo test -p rnote-ui retarget` — Expected: FAIL.

- [ ] **Step 3: Implementar** menús, diálogos, selección múltiple y el manejo de pestañas.

- [ ] **Step 4: Verificar**

Run: `cargo test -p rnote-ui && cargo test -p rnote-library && cargo clippy -p rnote-library -p rnote-ui --all-targets -- -D warnings && meson compile -C _mesonbuild`
Expected: verde. Manual en la copia: renombrar un cuaderno abierto, dibujar, guardar → se
guarda con el nombre nuevo y no queda el viejo; renombrar su carpeta → igual; mandarlo a la
papelera abierto → toast y nada se mueve; carpeta + etiqueta de color, mover conserva la
etiqueta, papelera/restaurar con nombre ocupado → "(2)"; `library.json` lo abre Android
(copiarlo a un `Library` de test o comparar el formato).

- [ ] **Step 5: Commit** — `feat: renombrar, mover, etiquetar, papelera y selección múltiple en la biblioteca de escritorio`

### Task 6: traducciones, Flatpak propio e instalación en la PC

**Files:**
- Create: `build-aux/io.github.yx14ng.rnote.json`, `build-aux/install-fork-flatpak.sh`,
  `build-aux/copy-official-settings.sh`
- Modify: `crates/rnote-ui/po/POTFILES`, `crates/rnote-ui/po/es.po`,
  `crates/rnote-ui/po/zh_CN.po`, `android/README.md` (sección corta "Desktop") o un
  `docs/fork-desktop.md` nuevo

**Interfaces:**
- Manifiesto: como `com.github.flxzt.rnote.Devel.json` pero `id` `io.github.yx14ng.rnote`,
  sin `tags`, con `--share=network`, `RUST_LOG=rnote=warn,...` (como el oficial), sin
  `RUST_BACKTRACE`, `config-opts` `-Dprofile=default -Dcli=false -Dapp-id=io.github.yx14ng.rnote -Dapp-name=Rnote (YX14ng)`,
  `run-tests: false`, fuente `dir` `../` con `skip` de `target`, `_mesonbuild`, `android`,
  `.flatpak-builder`, `_flatpak_build`.
- `install-fork-flatpak.sh`: instala si faltan (`flatpak install --user flathub
  org.flatpak.Builder org.gnome.Sdk//50 org.freedesktop.Sdk.Extension.rust-stable//25.08`),
  corre `flatpak run org.flatpak.Builder --user --install --force-clean _flatpak_build
  build-aux/io.github.yx14ng.rnote.json`, y si `RNOTE_LIBRARY_DIR` está, aplica
  `flatpak override --user --filesystem=$RNOTE_LIBRARY_DIR io.github.yx14ng.rnote` y
  guarda `library-dir`.
- `copy-official-settings.sh`: copia cada clave de `gsettings list-recursively` del oficial
  que exista en el esquema del fork (preset A4 oscuro, colores, grosores), sin tocar el
  oficial.

- [ ] **Step 1: Traducciones** de todos los textos nuevos (es, zh_CN), agregadas a mano a
  los `.po` (no se regenera `rnote.pot`: tocaría todo upstream), y `POTFILES`.
  Run: `msgfmt --check -o /dev/null crates/rnote-ui/po/es.po && msgfmt --check -o /dev/null crates/rnote-ui/po/zh_CN.po`
  Expected: sin errores.

- [ ] **Step 2: Commit** — `feat: traducciones al español y al chino de la biblioteca de escritorio`
  (Ruling: las traducciones son parte de la funcionalidad visible.)

- [ ] **Step 3: Manifiesto y scripts.** Run: `RNOTE_LIBRARY_DIR=/mnt/Storage/HandNotes build-aux/install-fork-flatpak.sh && build-aux/copy-official-settings.sh`
  Expected: `flatpak list --app` muestra `com.github.flxzt.rnote` 0.14.2 y
  `io.github.yx14ng.rnote` 0.15.0; `flatpak override --user --show io.github.yx14ng.rnote`
  incluye `/mnt/Storage/HandNotes`; `document-config-preset` del fork = el del oficial.

- [ ] **Step 4: Commit** — `chore: manifiesto Flatpak del fork y scripts de instalación`;
  docs aparte — `docs: cómo instalar el Rnote de escritorio del fork`.

- [ ] **Step 5: Prueba del usuario** sobre `/mnt/Storage/HandNotes` real (antes, el
  asistente sugiere guardar Orga desde Xournal++: solo hay `.Orga.autosave.xopp`). Push.
