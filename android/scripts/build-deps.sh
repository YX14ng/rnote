#!/usr/bin/env bash
# Cross-compiles the C dependencies of rnote-engine for Android and installs them,
# as static libraries with their pkg-config files, into $RNOTE_ANDROID_PREFIX.
#
# Usage: android/scripts/build-deps.sh [aarch64|x86_64]
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
deps_dir="$script_dir/../deps"
# shellcheck source=android/scripts/env.sh
. "$script_dir/env.sh" "${1:-aarch64}"

build_dir="$RNOTE_ANDROID_HOME/build/deps-$RNOTE_ANDROID_ABI"
cross_file="$RNOTE_ANDROID_HOME/build/cross-$RNOTE_ANDROID_ABI.ini"
mkdir -p "$build_dir" "$RNOTE_ANDROID_PREFIX"

sed \
  -e "s|@TOOLCHAIN@|$RNOTE_ANDROID_TOOLCHAIN|g" \
  -e "s|@TRIPLE@|$RNOTE_ANDROID_TRIPLE|g" \
  -e "s|@API@|$ANDROID_API|g" \
  -e "s|@ARCH@|$RNOTE_ANDROID_ARCH|g" \
  -e "s|@PREFIX@|$RNOTE_ANDROID_PREFIX|g" \
  "$script_dir/../meson/android-cross.ini.in" > "$cross_file"

meson_options=(
  --cross-file "$cross_file"
  --prefix "$RNOTE_ANDROID_PREFIX"
  --libdir lib
  --buildtype release
  --wrap-mode forcefallback
  -Ddefault_library=static
  -Db_staticpic=true

  -Dglib:tests=false
  -Dglib:introspection=disabled
  -Dglib:sysprof=disabled
  -Dglib:man-pages=disabled
  -Dglib:documentation=false
  -Dglib:selinux=disabled
  -Dglib:libmount=disabled
  -Dglib:nls=disabled

  -Dcairo:tests=disabled
  -Dcairo:xlib=disabled
  -Dcairo:xcb=disabled
  -Dcairo:quartz=disabled
  -Dcairo:dwrite=disabled
  -Dcairo:spectre=disabled
  -Dcairo:symbol-lookup=disabled
  -Dcairo:lzo=disabled
  -Dcairo:glib=enabled
  -Dcairo:png=enabled
  -Dcairo:zlib=enabled
  -Dcairo:freetype=enabled
  -Dcairo:fontconfig=enabled

  -Dpango:introspection=disabled
  -Dpango:sysprof=disabled
  -Dpango:xft=disabled
  -Dpango:libthai=disabled
  -Dpango:build-testsuite=false
  -Dpango:build-examples=false
  -Dpango:documentation=false
  -Dpango:man-pages=false
  -Dpango:fontconfig=enabled
  -Dpango:freetype=enabled
  -Dpango:cairo=enabled

  -Dharfbuzz:tests=disabled
  -Dharfbuzz:docs=disabled
  -Dharfbuzz:utilities=disabled
  -Dharfbuzz:introspection=disabled
  -Dharfbuzz:cairo=disabled
  -Dharfbuzz:icu=disabled
  -Dharfbuzz:glib=enabled
  -Dharfbuzz:freetype=enabled

  # harfbuzz depends on freetype; pango uses harfbuzz directly, so break the cycle here.
  -Dfreetype2:harfbuzz=disabled
  -Dfreetype2:brotli=disabled
  -Dfreetype2:bzip2=disabled
  -Dfreetype2:png=disabled

  # Static trampolines are wired up for Linux only; on Android the build ends up
  # referencing open_temp_exec_file without compiling it. glib only needs ffi_call.
  -Dlibffi:exe_static_tramp=false

  -Dfribidi:bin=false
  -Dfribidi:tests=false
  -Dfribidi:docs=false

  -Dlibxml2:icu=disabled
  -Dlibxml2:python=disabled

  -Dfontconfig:tests=disabled
  -Dfontconfig:tools=disabled
  -Dfontconfig:doc=disabled
  -Dfontconfig:cache-build=disabled
)

if [[ -f "$build_dir/build.ninja" ]]; then
  meson setup --reconfigure "${meson_options[@]}" "$build_dir" "$deps_dir"
else
  meson setup "${meson_options[@]}" "$build_dir" "$deps_dir"
fi
meson compile -C "$build_dir"
meson install -C "$build_dir" --quiet
echo "Installed Android deps for $RNOTE_ANDROID_ABI into $RNOTE_ANDROID_PREFIX"
