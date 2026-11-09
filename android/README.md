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
