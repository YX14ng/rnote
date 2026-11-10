# Rnote on Android

Work in progress: building `rnote-engine` (without the GTK `ui` feature) for Android, as
the base for a native Android frontend.

## Status

- The C libraries the engine links against are cross-compiled from pinned sources:
  glib/gio, cairo, pango/pangocairo, libxml2 and their dependencies.
- `rnote-engine` and `rnote-cli` build for `aarch64-linux-android` and `x86_64-linux-android`
  (API 28). Everything is linked statically: the binaries only need `libc`, `libm` and `libdl`.
- `rnote-cli` exports real `.rnote` documents to PDF on Android. The output is
  pixel-identical to the Linux build.
- `crates/rnote-android` is a JNI bridge over the engine, and `android/viewer` is an app built
  on it: open, create, edit and save documents. Files saved on Android open in the desktop
  app.

## Prerequisites

- Android NDK r30
- `rustup` with the `aarch64-linux-android` and/or `x86_64-linux-android` targets
- `meson` >= 1.4, `ninja`, `pkg-config`, network access for the first dependency build

By default everything lives in `~/.cache/rnote-android` (`RNOTE_ANDROID_HOME`): the NDK
in `toolchain/android-ndk-r30`, an optional isolated rustup install in `rustup/` and `cargo/`,
the build trees in `build/`, and the installed dependencies in `prefix/<abi>/`.
`ANDROID_NDK_HOME` and `ANDROID_API` can be overridden.

## Building

```sh
# 1. C dependencies (once per arch, ~5 min).
android/scripts/build-deps.sh aarch64

# 2. Rust, using the environment for the same arch.
. android/scripts/env.sh aarch64
cargo build -p rnote-cli --target aarch64-linux-android --release
```

Use `x86_64` instead of `aarch64` to target the emulator or Waydroid.

### Signing

Published APKs are signed with one stable key, so installed apps can be updated and the
Google Drive OAuth client, registered with the key's SHA-1, keeps working. Create it once,
with nothing to type:

```sh
android/scripts/create-keystore.sh   # ~/.config/rnote-android/release.jks + signing.env
```

It takes the key of this computer's debug keystore (`~/.android/debug.keystore`), which
signed the APKs built before, so installed apps keep updating; `RNOTE_NEW_KEY=1` generates
a new key instead. The password is random and goes, with the other variables the build
reads (`RNOTE_KEYSTORE`, `RNOTE_KEY_ALIAS`, `RNOTE_KEYSTORE_PASSWORD`,
`RNOTE_KEY_PASSWORD`), to `signing.env` (mode 600), which `build-apk.sh` loads (or the file
in `RNOTE_SIGNING_ENV`). Without it, debug builds use the debug key with a warning and
release builds refuse to build. Back both files up outside the computer: they can't be
recreated. Never commit them.

An app signed with another key must be uninstalled once before installing one signed with
the stable key; uninstalling offers to keep the app's data (`hasFragileUserData`), which
holds the notebook library.

## App

```sh
# Native library for both ABIs + debug APK. Needs JAVA_HOME pointing to a JDK 17-21
# (a JDK 21 in /usr/lib/jvm is picked up) and the SDK in ANDROID_HOME
# (default: ~/.cache/rnote-android/sdk, with platforms;android-36).
android/scripts/build-apk.sh
adb install -r android/viewer/app/build/outputs/apk/debug/app-debug.apk
```

`android/scripts/build-native.sh [aarch64] [x86_64]` only rebuilds `librnote_android.so`
into the app's `jniLibs`.

What it does:

- The app opens on a library of notebooks kept in its private storage
  (`filesDir/library`), like other note-taking apps: on the left all notes, uncategorized,
  folders and tags (each with a `+` to create one, long-press to rename or delete) and the
  trash, with counts; on the right a grid or list of the section's notebooks with a
  thumbnail of the first page with content, name and date, sorted by date or name. A
  "New" card creates "Untitled" in the folder shown. Each notebook's menu renames, moves to
  a folder, sets tags, duplicates or sends it to the trash; the trash restores (with a
  suffix if the name was taken) or deletes for good, and empties. "Select" acts on several
  notebooks at once. "Import" copies an `.rnote` or Xournal++ `.xopp` from the system
  picker; `.xopp` files in the library show as cards that open as the `.rnote` made from
  them. The library itself is `crates/rnote-library` (plain files, `library.json` for tags
  and the trash), shared with the desktop app and the Google Drive sync to come.
- Pages are outlined like in the desktop app, when the document has borders enabled.
- Opening fits one page to the screen width, starting at the origin page, and the zoom stays
  within the engine's limits (0.2 to 6). The engine's camera has to match the screen: the
  eraser only considers strokes inside it, and infinite documents grow to keep pages around
  it. Fitting the whole document instead made infinite documents grow on every edit on narrow
  (portrait) screens, and the eraser then missed strokes. The camera is set to exactly
  the area on screen (`Camera::set_offset_unclamped`), so the view can center a document
  smaller than the screen; past that it goes as far beyond the edges as the desktop app
  lets it (`Camera::surface_mins_maxs`). While a new frame renders after zooming out, the
  newly exposed part of the page is filled with its background color.
- Strokes follow the pointer that started them: a palm resting before, during or after a
  stylus stroke neither draws nor moves the page.
- Fingers navigate: drag and fling to pan, pinch or double tap to zoom. A mouse wheel pans,
  and zooms with Ctrl held.
- A stylus or a mouse uses the selected tool; the stylus eraser end and its primary button
  always erase. Fingers are ignored while the pen is down. Pressure is passed to the engine.
  With "Draw with finger", one finger draws too and two fingers navigate.
- The look ("Graphite") is dark neutral chrome, so the page stands out, with one blue
  accent; its design lives in a Claude Design canvas, and its colors in `Graphite.kt`.
- The editor's top bar has two rows. The first: back to the library, the notebook's name
  ("Unsaved" below it when needed; tap to rename a library notebook), "Finish shape" when
  needed, the zoom relative to the page width (tap to fit again), the menu and an arrow
  that folds the second row away. The second: undo and redo; the tools in the middle, with
  the desktop app's icons and order (converted to vector drawables from
  `crates/rnote-ui/data/icons`); five palette colors and the current width at the end.
  Tapping a color uses it; tapping the active one (marked with a small arrow) or
  long-pressing any opens its color menu below it, like other note-taking apps': a
  saturation and value square, hue and opacity bars, hex code and opacity, a preview with
  `+` to keep the color, and the kept colors (tap to use, long-press to forget). Changes
  apply at once. Tapping the active tool opens its options right below it:
  pen style, preset widths and switches; shapes and their style; text format and the
  selected text's emphasis; eraser mode; lasso actions.
- Tools: pen (marker, solid or textured brush style), shapes, text, lasso selection (move,
  resize and rotate through the engine's own handles) and eraser (whole strokes or only
  what it touches). What the lasso encloses gets a floating bar: copy, cut, duplicate,
  recolor with a palette color, delete. Shapes, the selection, the text cursor and the eraser outline are drawn
  by the engine.
- Shapes: line, arrow, rectangle, ellipse (from its center), grid and coordinate system with
  one drag; curves by dragging from the start to the control point (twice for two control
  points) and then tapping the end; polylines and polygons with one tap per point, finished
  by tapping the last point again or with "Finish shape". Hovering the stylus previews the
  next segment.
- Pen wheel: holding the stylus button in the air and releasing it without touching the
  screen opens a wheel at the pen: palette colors around it, and a dial inside that sets the
  width continuously (labelled ticks, a light snap and a haptic tick at the preset widths).
  The dial's track thickens along the way, the center previews the brush color and width,
  and the button at the bottom, the center or a tap outside close it.
- "Text format…": font (serif, sans serif, monospace, handwriting), size, bold, italic and
  alignment, for new text and the text being edited. Generic family names keep documents
  portable; the app's fontconfig setup maps them to Android's fonts and synthesizes bold and
  italic where a font lacks them.
- "Selected text": bold, italic, underline, strikethrough or the pen color on the selected
  part of the text being edited (select with Shift and the arrows, or "Select all text" /
  Ctrl+A), like the desktop app's per-range formatting.
- "Recognize shapes" (off by default): holding the pen still for a second at the end of a
  pen stroke that looks like a shape turns it into that shape: a line, rectangle, ellipse,
  triangle, any polygon with sharp corners (pentagons, hexagons, rhombi, stars, L shapes,
  ...), an arrow drawn in one stroke (shaft, then out to each barb and back), or a polyline
  (zigzags). Nearly regular polygons and five-pointed stars are made regular. The shape is
  styled like the shape tool draws, in one undo step; the rest of that touch is ignored.
  Nearly straight or square ones are straightened, and nearly round ones made round. Lifting
  the pen without holding keeps the stroke as drawn. The recognizer is
  `crates/rnote-android/src/shape_recognition/`; the engine got `Engine::insert_shapes` to
  put the shape in place of the stroke.
- A quick tap with two fingers undoes, like in other note-taking apps.
- "Document…": page size (A3, A4, A5, Letter, Legal, or any width and height in mm) and
  orientation, layout (fixed pages, continuous vertical, semi-infinite, infinite),
  background color (presets or any hex color), pattern, pattern color and spacing, and page
  borders. Like on the desktop, these don't go through undo.
- Cut, copy and paste through the system clipboard: selected strokes as Rnote stroke content
  (pasted back as strokes, also when a synced clipboard turned them into plain text), the
  selected text as plain text. Pasting also takes text and images from other apps. Pasted
  content lands in the middle of the view, selected.
- The menu is a panel in sections: cut, copy and paste; file; document; input. Hardware
  keyboard shortcuts (`KeyShortcuts.kt`): Ctrl+Z undo, Ctrl+Shift+Z or Ctrl+Y redo,
  Ctrl+X/C/V, Ctrl+A select all, Ctrl+N new notebook beside the open one, Ctrl+O open,
  Ctrl+S save.
- Text: tapping with the text tool places a text box and shows the keyboard; the canvas
  shrinks above the keyboard and keeps the typing spot visible. Soft keyboards
  go through `TextInputConnection` (composing text is typed live and replaced as it
  changes); hardware keys, including arrows, Home/End and Enter, through `KeyMapping`.
- Until a color is picked, the pen is black or white depending on the background. The
  choices are remembered across documents and restarts.
- Back (the arrow or the system gesture) saves a library notebook before returning to the
  library, and stays in the editor if saving fails. Open, Save (overwrites the opened file)
  and Save as go through the system file picker, which starts in the folder of the open
  file or else in the shared Documents folder; leaving or replacing a file opened that way
  with unsaved changes asks first.
- Autosave: when the app goes to the background with unsaved changes, it saves to the open
  file, or keeps a recovery copy (in the app's private storage) for documents without one
  or that are read-only. The copy is restored on the next start.

How it fits together:

- `crates/rnote-android/src/rnote_document.rs`: `RnoteDocument`, plain Rust and tested on
  the host. Loads or creates a document, feeds it pen samples, undoes, redoes and saves.
- `render.rs`: renders any viewport to premultiplied RGBA. The background is drawn by
  repeating a cached tile, like the desktop app: rendering it as one SVG took seconds when
  zoomed out.
- `pen_input.rs`: turns stylus samples into the engine's `PenEvent`s.
- Tool changes go through `Engine::change_pen_style` after switching the engine to the pen
  side: style changes apply to the current pen mode, which is the eraser after using the
  stylus' eraser end.
- `crates/rnote-android/src/jni_bridge/` and `android_bitmap.rs`: the JNI entry points and
  the NDK bitmap access. All `unsafe` lives there, with the invariants the Kotlin side
  upholds documented in the module docs.
- `android/viewer`: Kotlin, framework APIs only (no AndroidX). `DocumentRenderer` confines
  every native call to one thread; `NoteCanvasView` draws the last frame transformed to the
  current viewport while the next one renders.
- While the engine renders, `NoteCanvasView` previews the stroke being drawn in Kotlin, so
  ink shows up without waiting for a frame.
- The app sets `FONTCONFIG_FILE` itself at startup (see below).

To debug rendering on the host:

```sh
cargo run -p rnote-android --example render_region -- note.rnote <x> <y> <zoom> <w> <h> out.rgba
magick -size <w>x<h> -depth 8 rgba:out.rgba out.png
```

## Running

fontconfig is built with a config path inside the build prefix, which doesn't exist on the
device. Point it at the system fonts and a writable cache:

```xml
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
  <dir>/system/fonts</dir>
  <cachedir>/data/local/tmp/rnote/fc-cache</cachedir>
</fontconfig>
```

```sh
FONTCONFIG_FILE=/data/local/tmp/rnote/fonts.conf ./rnote-cli export doc -o note.pdf note.rnote
```

## Notes on the dependency build

- `android/deps/` is a meson superproject. Every dependency comes from a wrap in
  `subprojects/` pinned to an exact version and sha256, and `--wrap-mode=forcefallback`
  keeps host libraries out.
- pango uses the upstream 1.58.2 tarball instead of WrapDB's 1.56.4, which doesn't build
  against fontconfig 2.18.
- freetype is built without harfbuzz to break the freetype <-> harfbuzz cycle.
- libffi static trampolines are disabled: on Android its meson build references
  `open_temp_exec_file` without compiling it.
- ICU is disabled in harfbuzz and libxml2; Rnote doesn't need it.
- `env.sh` links compiler-rt's builtins explicitly, because rustc links Android targets
  with `-nodefaultlibs` and the libffi/pcre2 JITs need `__clear_cache`.

## Known issues

- Exporting some documents to SVG used to fail with `BackendError(InvalidMatrix)`, on Linux
  too: an image whose placement had collapsed to zero size. Such images are now skipped when
  drawn; `image_scaled_to_nothing_does_not_break_svg_export` covers it.
- Background patterns used to vanish once the drawn region was more than ~32k device
  pixels away from the document origin (pixman's 16 bit limit), e.g. far down a long
  document when zoomed in. Fixed in the engine by anchoring each pattern next to the drawn
  region; `renders_pattern_far_from_document_origin` covers it.
- Keyboards get no suggestions, since the engine owns the text and the keyboard can't read it back. The
  desktop "tools" pen (offsetting, zooming) isn't exposed.
- A `NullPointerException` showed up once during testing and couldn't be reproduced in six
  later attempts, three of them from a clean install.
- The clipboard keeps one format per copy: strokes don't also go out as an image for other
  apps.
- Edge-to-edge insets (enforced on Android 15+) are handled but untested: the test device
  (Waydroid) runs Android 13.
