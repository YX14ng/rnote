#!/usr/bin/env bash
# Builds librnote_android.so and copies it into the viewer app's jniLibs.
#
# Usage: android/scripts/build-native.sh [aarch64] [x86_64]   (default: both)
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_dir="$(cd "$script_dir/../.." && pwd)"
jni_libs_dir="$script_dir/../viewer/app/src/main/jniLibs"

arches=("$@")
if [[ ${#arches[@]} -eq 0 ]]; then
  arches=(aarch64 x86_64)
fi

for arch in "${arches[@]}"; do
  (
    # shellcheck source=android/scripts/env.sh
    . "$script_dir/env.sh" "$arch"
    if [[ ! -f "$RNOTE_ANDROID_PREFIX/lib/pkgconfig/pangocairo.pc" ]]; then
      "$script_dir/build-deps.sh" "$arch"
    fi
    # No debug info and no static symbol table: the library goes into an APK. The JNI
    # entry points live in the dynamic symbol table, which stripping keeps.
    CARGO_PROFILE_RELEASE_DEBUG=false CARGO_PROFILE_RELEASE_STRIP=symbols \
      cargo build --manifest-path "$repo_dir/Cargo.toml" -p rnote-android \
      --target "$RNOTE_ANDROID_TRIPLE" --release
    mkdir -p "$jni_libs_dir/$RNOTE_ANDROID_ABI"
    cp "$repo_dir/target/$RNOTE_ANDROID_TRIPLE/release/librnote_android.so" \
      "$jni_libs_dir/$RNOTE_ANDROID_ABI/"
    echo "Copied librnote_android.so for $RNOTE_ANDROID_ABI"
  )
done
