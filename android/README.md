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
- `crates/rnote-android` is a JNI bridge over the engine, and `android/viewer` is a minimal
  read-only viewer app built on it: open a file, pan, pinch/double-tap zoom, fling.

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

## Viewer app

```sh
# Native library for both ABIs + debug APK. Needs JAVA_HOME pointing to a JDK 17-21
# (a JDK 21 in /usr/lib/jvm is picked up) and the SDK in ANDROID_HOME
# (default: ~/.cache/rnote-android/sdk, with platforms;android-36).
android/scripts/build-apk.sh
adb install -r android/viewer/app/build/outputs/apk/debug/app-debug.apk
```

`android/scripts/build-native.sh [aarch64] [x86_64]` only rebuilds `librnote_android.so`
into the app's `jniLibs`.

How it fits together:

- `crates/rnote-android/src/document_view.rs`: `DocumentView`, plain Rust and tested on the
  host. Loads an `.rnote` and renders any viewport to premultiplied RGBA.
- `crates/rnote-android/src/jni_bridge.rs` and `android_bitmap.rs`: the JNI entry points and
  the NDK bitmap access. All `unsafe` lives there, with the invariants the Kotlin side
  upholds documented in the module docs.
- `android/viewer`: Kotlin, framework APIs only (no AndroidX). `DocumentRenderer` confines
  every native call to one thread; `NoteCanvasView` draws the last frame transformed to the
  current viewport while the next one renders.
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
- The viewer only shows the document; there is no page separation, editing or stylus
  input yet.
- Edge-to-edge insets (enforced on Android 15+) are handled but untested: the test device
  (Waydroid) runs Android 13.
