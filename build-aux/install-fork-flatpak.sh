#!/usr/bin/env bash
# Builds the fork's Flatpak (io.github.yx14ng.rnote, "Rnote (YX14ng)") and installs it for
# the current user, beside Flathub's Rnote, which stays untouched.
#
# --bundle FILE: builds a single-file bundle for a release instead (`flatpak install
# FILE` installs it, with the GNOME runtime from Flathub), and installs nothing here. A
# bundle is for other people, so it refuses to build without the Drive OAuth client.
#
# RNOTE_LIBRARY_DIR (optional): the library folder; the app gets read and write access to
# it and uses it as its library.
#
# Google Drive sync needs the "Desktop app" OAuth client of the Google Cloud project, at
# build time: RNOTE_GDRIVE_CLIENT_ID and RNOTE_GDRIVE_CLIENT_SECRET, or else the client's
# JSON as Google downloads it, at RNOTE_GDRIVE_CLIENT_JSON (default
# ~/.config/rnote-android/desktop-client.json). They go into a private file outside the
# repository, never into a tracked file, which the build reads (build-aux/cargo_build.py).
# Without them the app builds with the sync off.
set -euo pipefail

bundle=
case "${1:-}" in
  --bundle)
    if [[ -z "${2:-}" ]]; then
      echo "usage: $0 [--bundle FILE]" >&2
      exit 2
    fi
    bundle=$(realpath -m -- "$2")
    ;;
  "") ;;
  *)
    echo "usage: $0 [--bundle FILE]" >&2
    exit 2
    ;;
esac

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
  if [[ -n "$bundle" ]]; then
    echo "No Google Drive OAuth client: a release bundle needs one (see the top of $0)." >&2
    exit 1
  fi
  echo "No Google Drive OAuth client: building with the sync turned off." >&2
fi

# The manifest with the source as an absolute path, and the credentials beside it, in a
# private folder that is removed at the end. Not in /tmp: Flatpak Builder
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
keys = ("RNOTE_GDRIVE_CLIENT_ID", "RNOTE_GDRIVE_CLIENT_SECRET")
if all(os.environ.get(key) for key in keys):
    # A file source, not the build environment: flatpak-builder copies the manifest into
    # the app (/app/manifest.json), and a file source shows there only as its path.
    client = os.path.join(os.path.dirname(target), "gdrive-client.env")
    with open(os.open(client, os.O_WRONLY | os.O_CREAT, 0o600), "w") as out:
        out.writelines(f"{key}={os.environ[key]}\n" for key in keys)
    for module in manifest["modules"]:
        if module.get("name") == "rnote":
            module["sources"].append(
                {"type": "file", "path": client, "dest-filename": "gdrive-client.env"})
with open(os.open(target, os.O_WRONLY | os.O_CREAT, 0o600), "w") as out:
    json.dump(manifest, out, indent=2)
PY

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
if ! flatpak info org.flatpak.Builder >/dev/null 2>&1; then
  flatpak install --user --noninteractive flathub org.flatpak.Builder
fi
if [[ -n "$bundle" ]]; then
  flatpak run org.flatpak.Builder --user --force-clean --repo="$work/repo" \
    --install-deps-from=flathub _flatpak_build "$work/$app.json"
  flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    "$work/repo" "$bundle" "$app"
  echo "Bundle: $bundle"
  exit 0
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
