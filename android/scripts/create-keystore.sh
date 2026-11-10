#!/usr/bin/env bash
# Creates the stable keystore that signs every APK, and the file with the variables that
# point the build at it. Nothing to type: the password is random and kept in that file.
#
# Usage: android/scripts/create-keystore.sh [keystore path]
#   (default: ~/.config/rnote-android/release.jks; the variables go to signing.env beside it)
#
# It takes the key of this computer's debug keystore (~/.android/debug.keystore), which
# signs the APKs built so far, so installed apps keep updating without being uninstalled.
# Set RNOTE_NEW_KEY=1 to generate a new key instead. Back the keystore and signing.env up
# outside this computer: without them, installed apps can't be updated.
set -euo pipefail

keystore="${1:-$HOME/.config/rnote-android/release.jks}"
signing_env="$(dirname "$keystore")/signing.env"
debug_keystore="$HOME/.android/debug.keystore"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [[ -e "$keystore" || -e "$signing_env" ]]; then
  echo "create-keystore.sh: '$keystore' or '$signing_env' already exists, not touching them" >&2
  exit 1
fi
keystore_dir="$(mkdir -p "$(dirname "$keystore")" && cd "$(dirname "$keystore")" && pwd)"
if [[ "$keystore_dir/" == "$repo_root/"* ]]; then
  echo "create-keystore.sh: the keystore must live outside the repository" >&2
  exit 1
fi

umask 077
# keytool reads the password from the environment, so it never shows in the process list.
RNOTE_KEYSTORE_PASSWORD="$(head -c 48 /dev/urandom | base64 | tr -dc 'A-Za-z0-9' | head -c 32)"
export RNOTE_KEYSTORE_PASSWORD

if [[ -f "$debug_keystore" && -z "${RNOTE_NEW_KEY:-}" ]]; then
  # The debug keystore's password is Android's well-known "android".
  keytool -importkeystore -noprompt \
    -srckeystore "$debug_keystore" -srcstorepass android -srcalias androiddebugkey \
    -destkeystore "$keystore" -deststoretype PKCS12 -destalias rnote \
    -deststorepass:env RNOTE_KEYSTORE_PASSWORD -destkeypass:env RNOTE_KEYSTORE_PASSWORD
  echo "Took the key of $debug_keystore: installed apps keep updating."
else
  keytool -genkeypair -noprompt -keystore "$keystore" -storetype PKCS12 -alias rnote \
    -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=Rnote Android" \
    -storepass:env RNOTE_KEYSTORE_PASSWORD -keypass:env RNOTE_KEYSTORE_PASSWORD
  echo "Generated a new key: apps signed with another key must be uninstalled once."
fi

cat >"$signing_env" <<ENV
# Loaded by android/scripts/build-apk.sh. Written by create-keystore.sh; keep it private.
export RNOTE_KEYSTORE="$keystore"
export RNOTE_KEY_ALIAS=rnote
export RNOTE_KEYSTORE_PASSWORD='$RNOTE_KEYSTORE_PASSWORD'
export RNOTE_KEY_PASSWORD='$RNOTE_KEYSTORE_PASSWORD'
ENV

keytool -list -v -keystore "$keystore" -alias rnote -storepass:env RNOTE_KEYSTORE_PASSWORD \
  | grep -E "SHA1|SHA256"
echo
echo "Back up $keystore and $signing_env outside this computer."
