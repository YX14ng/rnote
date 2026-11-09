#!/usr/bin/env bash
# Builds the native library and the debug APK of the viewer app.
#
# Usage: android/scripts/build-apk.sh [aarch64] [x86_64]   (default: both)
#
# Needs JAVA_HOME pointing to a JDK 17-21 (a JDK 21 in /usr/lib/jvm is picked up if
# unset) and the Android SDK in ANDROID_HOME (default: $RNOTE_ANDROID_HOME/sdk).
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
"$script_dir/build-native.sh" "$@"

rnote_android_home="${RNOTE_ANDROID_HOME:-$HOME/.cache/rnote-android}"
export ANDROID_HOME="${ANDROID_HOME:-$rnote_android_home/sdk}"
if [[ ! -d "$ANDROID_HOME/platforms" ]]; then
  echo "build-apk.sh: Android SDK not found at '$ANDROID_HOME'" >&2
  exit 1
fi
if [[ -z "${JAVA_HOME:-}" && -d /usr/lib/jvm/java-21-openjdk ]]; then
  export JAVA_HOME=/usr/lib/jvm/java-21-openjdk
fi
if [[ -z "${JAVA_HOME:-}" ]]; then
  echo "build-apk.sh: set JAVA_HOME to a JDK 17-21" >&2
  exit 1
fi

cd "$script_dir/../viewer"
./gradlew --no-daemon assembleDebug
echo "APK: $(pwd)/app/build/outputs/apk/debug/app-debug.apk"
