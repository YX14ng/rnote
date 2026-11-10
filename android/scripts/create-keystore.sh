#!/usr/bin/env bash
# Creates the stable keystore that signs every APK, and the file with the variables that
# point the build at it.
#
# Usage: android/scripts/create-keystore.sh [keystore path]
#   (default: ~/.config/rnote-android/release.jks)
#
# keytool asks for the passwords; they are never passed as arguments. Back the keystore up
# outside this computer: without it, installed apps can't be updated.
set -euo pipefail

keystore="${1:-$HOME/.config/rnote-android/release.jks}"
signing_env="$(dirname "$keystore")/signing.env"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [[ -e "$keystore" ]]; then
  echo "create-keystore.sh: '$keystore' already exists, not touching it" >&2
  exit 1
fi
keystore_dir="$(mkdir -p "$(dirname "$keystore")" && cd "$(dirname "$keystore")" && pwd)"
if [[ "$keystore_dir/" == "$repo_root/"* ]]; then
  echo "create-keystore.sh: the keystore must live outside the repository" >&2
  exit 1
fi

keytool -genkeypair -v -keystore "$keystore" -alias rnote -keyalg RSA -keysize 4096 \
  -validity 10000 -dname "CN=Rnote Android"

if [[ ! -e "$signing_env" ]]; then
  # The passwords are typed into this file by hand, never written by a script.
  cat >"$signing_env" <<ENV
# Loaded by android/scripts/build-apk.sh. Fill in the passwords typed into keytool.
export RNOTE_KEYSTORE="$keystore"
export RNOTE_KEY_ALIAS=rnote
export RNOTE_KEYSTORE_PASSWORD=
export RNOTE_KEY_PASSWORD=
ENV
  chmod 600 "$signing_env"
fi

keytool -list -v -keystore "$keystore" -alias rnote | grep -E "SHA1|SHA256"
echo
echo "Now write the passwords into $signing_env"
echo "and back up $keystore outside this computer."
