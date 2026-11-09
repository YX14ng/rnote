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

- Pages are outlined like in the desktop app, when the document has borders enabled.
- Fingers navigate: drag and fling to pan, pinch or double tap to zoom. A mouse wheel pans,
  and zooms with Ctrl held.
- A stylus or a mouse uses the selected tool; the stylus eraser end and its primary button
  always erase. Fingers are ignored while the pen is down. Pressure is passed to the engine.
  With "Draw with finger", one finger draws too and two fingers navigate.
- Tools: pen (marker, solid or textured brush style), shapes, text, selection (lasso, move, resize and rotate through the
  engine's own handles; select all, duplicate, delete) and eraser (whole strokes or only
  what it touches). Shapes, the selection, the text cursor and the eraser outline are drawn
  by the engine.
- Shapes: line, arrow, rectangle, ellipse (from its center), grid and coordinate system with
  one drag; curves by dragging from the start to the control point (twice for two control
  points) and then tapping the end; polylines and polygons with one tap per point, finished
  by tapping the last point again or with "Finish shape". Hovering the stylus previews the
  next segment.
- "Text format…": font (serif, sans serif, monospace, handwriting), size, bold, italic and
  alignment, for new text and the text being edited. Generic family names keep documents
  portable; the app's fontconfig setup maps them to Android's fonts and synthesizes bold and
  italic where a font lacks them.
- "Shape style…": smooth or hand-drawn (rough) shapes, with an optional fill.
- "Document…": page size (A3, A4, A5, Letter, Legal) and orientation, layout (fixed pages,
  continuous vertical, semi-infinite, infinite), background color and pattern, and page
  borders. Like on the desktop, these don't go through undo.
- Text: tapping with the text tool places a text box and shows the keyboard; the canvas
  shrinks above the keyboard and keeps the typing spot visible. Soft keyboards
  go through `TextInputConnection` (composing text is typed live and replaced as it
  changes); hardware keys, including arrows, Home/End and Enter, through `KeyMapping`.
- Undo and redo, color (6 presets, black or white by default depending on the background)
  and width (3 presets). The choices are remembered across documents and restarts.
- New, Open, Save (overwrites the opened file) and Save as, through the system file picker.
  Unsaved changes are marked in the title, and leaving or replacing the document asks first.
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
- `crates/rnote-android/src/jni_bridge.rs` and `android_bitmap.rs`: the JNI entry points and
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

- Exporting some documents to SVG fails with `BackendError(InvalidMatrix)`. This also
  happens on Linux with the same document, so it isn't specific to Android.
- Background patterns used to vanish once the drawn region was more than ~32k device
  pixels away from the document origin (pixman's 16 bit limit), e.g. far down a long
  document when zoomed in. Fixed in the engine by anchoring each pattern next to the drawn
  region; `renders_pattern_far_from_document_origin` covers it.
- Formatting applies to a whole text box: no per-word ranges as on the desktop. Keyboards get
  no suggestions, since the engine owns the text and the keyboard can't read it back. The
  desktop "tools" pen (offsetting, zooming) isn't exposed.
- Presets only: no free page size, background color or pattern size and color.
- The selection offers no copy and paste between documents or apps.
- Edge-to-edge insets (enforced on Android 15+) are handled but untested: the test device
  (Waydroid) runs Android 13.
