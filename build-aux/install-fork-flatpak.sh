#!/usr/bin/env bash
# Builds the fork's Flatpak (io.github.yx14ng.rnote, "Rnote (YX14ng)") and installs it for
# the current user, beside Flathub's Rnote, which stays untouched.
#
# RNOTE_LIBRARY_DIR (optional): the library folder; the app gets read and write access to
# it and uses it as its library.
#
# Google Drive sync needs the "Desktop app" OAuth client of the Google Cloud project, at
# build time: RNOTE_GDRIVE_CLIENT_ID and RNOTE_GDRIVE_CLIENT_SECRET, or else the client's
# JSON as Google downloads it, at RNOTE_GDRIVE_CLIENT_JSON (default
# ~/.config/rnote-android/desktop-client.json). They go into a copy of the manifest outside
# the repository, never into a tracked file. Without them the app builds with the sync off.
set -euo pipefail

cd "$(dirname "$0")/.."
app=io.github.yx14ng.rnote
manifest=build-aux/$app.json

client_json=${RNOTE_GDRIVE_CLIENT_JSON:-$HOME/.config/rnote-android/desktop-client.json}
if [[ -z "${RNOTE_GDRIVE_CLIENT_ID:-}" || -z "${RNOTE_GDRIVE_CLIENT_SECRET:-}" ]] &&
  [[ -f "$client_json" ]]; then
  read_client() {
    python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["installed"][sys.argv[2]])' \
      "$client_json" "$1"
  }
  RNOTE_GDRIVE_CLIENT_ID=$(read_client client_id)
  RNOTE_GDRIVE_CLIENT_SECRET=$(read_client client_secret)
fi
export RNOTE_GDRIVE_CLIENT_ID=${RNOTE_GDRIVE_CLIENT_ID:-}
export RNOTE_GDRIVE_CLIENT_SECRET=${RNOTE_GDRIVE_CLIENT_SECRET:-}
if [[ -z "$RNOTE_GDRIVE_CLIENT_ID" || -z "$RNOTE_GDRIVE_CLIENT_SECRET" ]]; then
  echo "No Google Drive OAuth client: building with the sync turned off." >&2
fi

# The manifest with the source as an absolute path and the credentials in its build
# environment, in a private folder that is removed at the end. Not in /tmp: Flatpak Builder
# runs in a sandbox of its own that has another /tmp.
work=$(mktemp -d "${XDG_CACHE_HOME:-$HOME/.cache}/rnote-flatpak.XXXXXX")
trap 'rm -rf "$work"' EXIT
chmod 700 "$work"
python3 - "$manifest" "$work/$app.json" "$PWD" <<'PY'
import json, os, sys

source, target, repo = sys.argv[1:]
manifest = json.load(open(source))
for module in manifest["modules"]:
    for src in module.get("sources", []):
        if src.get("type") == "dir":
            src["path"] = repo
env = {key: os.environ[key] for key in ("RNOTE_GDRIVE_CLIENT_ID", "RNOTE_GDRIVE_CLIENT_SECRET")
       if os.environ.get(key)}
if env:
    manifest.setdefault("build-options", {}).setdefault("env", {}).update(env)
with open(os.open(target, os.O_WRONLY | os.O_CREAT, 0o600), "w") as out:
    json.dump(manifest, out, indent=2)
PY

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
if ! flatpak info org.flatpak.Builder >/dev/null 2>&1; then
  flatpak install --user --noninteractive flathub org.flatpak.Builder
fi
flatpak run org.flatpak.Builder --user --install --force-clean \
  --install-deps-from=flathub _flatpak_build "$work/$app.json"

if [[ -n "${RNOTE_LIBRARY_DIR:-}" ]]; then
  if [[ ! -d "$RNOTE_LIBRARY_DIR" ]]; then
    echo "RNOTE_LIBRARY_DIR is not a folder: $RNOTE_LIBRARY_DIR" >&2
    exit 1
  fi
  flatpak override --user --filesystem="$RNOTE_LIBRARY_DIR" "$app"
  flatpak run --command=gsettings "$app" set "$app" library-dir "$RNOTE_LIBRARY_DIR"
fi
