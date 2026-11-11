#!/usr/bin/env bash
# A gcloud of its own for the Google Cloud project behind the Drive sync, kept apart from
# the system: the SDK goes to one folder and its settings and sign-in to another, nothing
# is added to PATH or the shell's startup files, and deleting both folders removes it all.
#
# Usage: android/scripts/gcloud.sh <command>
#   install            downloads the SDK (also done by any other command when it's missing)
#   login              signs in with a Google account, in the browser
#   setup [project]    creates the project (or reuses the one set up before), enables the
#                      Drive API and prints what the console steps need
#   uninstall          deletes the SDK and its settings, sign-in included
#   <gcloud arguments> runs gcloud itself, e.g. `android/scripts/gcloud.sh projects list`
#
# Where: the SDK in RNOTE_GCLOUD_HOME (default ~/.local/opt/rnote-gcloud), the settings in
# RNOTE_GCLOUD_CONFIG (default ~/.config/rnote-android/gcloud), never inside the repository:
# they hold the account's tokens.
set -euo pipefail

sdk_home="${RNOTE_GCLOUD_HOME:-$HOME/.local/opt/rnote-gcloud}"
export CLOUDSDK_CONFIG="${RNOTE_GCLOUD_CONFIG:-$HOME/.config/rnote-android/gcloud}"
signing_env="${RNOTE_SIGNING_ENV:-$HOME/.config/rnote-android/signing.env}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
gcloud_bin="$sdk_home/google-cloud-sdk/bin/gcloud"
archive_url="https://dl.google.com/dl/cloudsdk/channels/rapid/downloads/google-cloud-cli-linux-x86_64.tar.gz"
package_name="io.github.yx14ng.rnote.viewer"

die() {
  echo "gcloud.sh: $*" >&2
  exit 1
}

# Fails unless `$1` is an absolute path outside the repository and isn't $HOME itself.
check_place() {
  local path="$1"
  [[ "$path" == /* ]] || die "'$path' must be an absolute path"
  [[ "${path%/}" != "${HOME%/}" && "$path" != "/" ]] || die "refusing to use '$path'"
  [[ "$path/" != "$repo_root/"* ]] || die "'$path' must live outside the repository"
}

install_sdk() {
  if [[ -x "$gcloud_bin" ]]; then
    return
  fi
  [[ "$(uname -m)" == x86_64 ]] || die "only x86_64 is handled, this is $(uname -m)"
  mkdir -p "$sdk_home"
  local download
  download="$(mktemp -d "$sdk_home/.download.XXXXXX")"
  trap 'rm -rf "$download"' EXIT
  echo "Downloading the Google Cloud CLI to $sdk_home ..."
  curl -fL --proto '=https' --tlsv1.2 -o "$download/sdk.tar.gz" "$archive_url"
  tar -xzf "$download/sdk.tar.gz" -C "$download"
  mv "$download/google-cloud-sdk" "$sdk_home/"
  rm -rf "$download"
  trap - EXIT
  # The archive's install.sh isn't run: it edits the shell's startup files.
  umask 077
  mkdir -p "$CLOUDSDK_CONFIG"
  "$gcloud_bin" config set core/disable_usage_reporting true --quiet
  "$gcloud_bin" config set component_manager/disable_update_check true --quiet
  "$gcloud_bin" --version | head -1
}

gcloud() {
  "$gcloud_bin" "$@"
}

signed_in() {
  [[ -n "$(gcloud auth list --filter=status:ACTIVE --format='value(account)' 2>/dev/null)" ]]
}

# The SHA-1 of the stable signing key, which the Android OAuth client is registered with.
signing_sha1() {
  [[ -f "$signing_env" ]] || return 0
  # shellcheck source=/dev/null
  (. "$signing_env" && keytool -list -v -keystore "$RNOTE_KEYSTORE" -alias "$RNOTE_KEY_ALIAS" \
    -storepass:env RNOTE_KEYSTORE_PASSWORD 2>/dev/null) | sed -n 's/^.*SHA1: *//p'
}

setup() {
  signed_in || die "sign in first: android/scripts/gcloud.sh login"
  local project="${1:-${RNOTE_GCP_PROJECT:-}}"
  if [[ -z "$project" ]]; then
    project="$(gcloud config get-value core/project 2>/dev/null || true)"
  fi
  if [[ -z "$project" ]]; then
    # Project ids are global across Google Cloud: a random tail keeps it free.
    project="rnote-sync-$(head -c 3 /dev/urandom | od -An -tx1 | tr -d ' \n')"
  fi
  if gcloud projects describe "$project" --format='value(projectId)' >/dev/null 2>&1; then
    echo "Using the project $project."
  else
    echo "Creating the project $project ..."
    local output
    if ! output="$(gcloud projects create "$project" --name="Rnote" 2>&1)"; then
      echo "$output" >&2
      # Projects being deleted keep counting for 30 days, so deleting one doesn't help.
      if grep -q "project quota" <<<"$output"; then
        die "the account has no room for another project: reuse one with" \
          "'android/scripts/gcloud.sh setup <project id>' (ids in 'gcloud.sh projects list')"
      fi
      die "couldn't create it. If it's about the terms of service, open" \
        "https://console.cloud.google.com once, accept them and run this again."
    fi
  fi
  gcloud config set core/project "$project" --quiet
  echo "Enabling the Google Drive API ..."
  # A project just created or restored takes a minute or two to show up for this.
  local try
  for try in 1 2 3 4 5 6; do
    gcloud services enable drive.googleapis.com --project="$project" 2>/dev/null && break
    [[ "$try" -lt 6 ]] || gcloud services enable drive.googleapis.com --project="$project"
    sleep 30
  done

  local sha1 console="https://console.cloud.google.com/auth"
  sha1="$(signing_sha1)"
  cat <<STEPS

Done here. The rest only exists in the web console (signed in with the same account):

1. Branding:  $console/overview?project=$project
   "Get started": app name Rnote, your email as support and contact, audience External.
2. Data access:  $console/scopes?project=$project
   "Add or remove scopes": .../auth/drive.file (see, edit, create and delete only the
   Google Drive files you use with this app). It's non-sensitive: no review by Google.
3. Clients:  $console/clients/create?project=$project
   a) Android: package name $package_name
      SHA-1 certificate fingerprint ${sha1:-(not found: run android/scripts/create-keystore.sh)}
   b) Desktop app: any name, e.g. "Rnote desktop".
4. Audience:  $console/audience?project=$project
   "Publish app", so any Google account can sign in. In Testing, only the listed test
   users can, and their sign-in expires after 7 days.

The client ids aren't secret, but don't commit the desktop client's secret: keep it in
~/.config/rnote-android/.
STEPS
}

command="${1:-}"
[[ -n "$command" ]] || { sed -n '2,16p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 1; }
check_place "$sdk_home"
check_place "$CLOUDSDK_CONFIG"
case "$command" in
  install)
    install_sdk
    ;;
  login)
    install_sdk
    gcloud auth login
    ;;
  setup)
    install_sdk
    shift
    setup "$@"
    ;;
  uninstall)
    # Never with the signing key: it can't be recreated.
    [[ "$signing_env" != "${CLOUDSDK_CONFIG%/}/"* && "$signing_env" != "${sdk_home%/}/"* ]] \
      || die "'$CLOUDSDK_CONFIG' or '$sdk_home' holds $signing_env, not deleting it"
    rm -rf "$sdk_home" "$CLOUDSDK_CONFIG"
    echo "Deleted $sdk_home and $CLOUDSDK_CONFIG."
    ;;
  *)
    install_sdk
    gcloud "$@"
    ;;
esac
