#!/usr/bin/env bash
# Builds the fork's Flatpak (io.github.yx14ng.rnote, "Rnote (YX14ng)") and installs it for
# the current user, beside Flathub's Rnote, which stays untouched.
#
# RNOTE_LIBRARY_DIR (optional): the library folder; the app gets read and write access to
# it and uses it as its library.
set -euo pipefail

cd "$(dirname "$0")/.."
app=io.github.yx14ng.rnote
manifest=build-aux/$app.json

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
if ! flatpak info org.flatpak.Builder >/dev/null 2>&1; then
  flatpak install --user --noninteractive flathub org.flatpak.Builder
fi
flatpak run org.flatpak.Builder --user --install --force-clean \
  --install-deps-from=flathub _flatpak_build "$manifest"

if [[ -n "${RNOTE_LIBRARY_DIR:-}" ]]; then
  if [[ ! -d "$RNOTE_LIBRARY_DIR" ]]; then
    echo "RNOTE_LIBRARY_DIR is not a folder: $RNOTE_LIBRARY_DIR" >&2
    exit 1
  fi
  flatpak override --user --filesystem="$RNOTE_LIBRARY_DIR" "$app"
  flatpak run --command=gsettings "$app" set "$app" library-dir "$RNOTE_LIBRARY_DIR"
fi
