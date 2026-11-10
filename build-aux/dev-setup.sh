#!/usr/bin/env bash
# Configures the fork's development build in _mesonbuild, under its own app id so it never
# mixes with an installed Rnote. Also generates crates/rnote-ui/src/config.rs, which
# `cargo test -p rnote-ui` needs.
set -euo pipefail

cd "$(dirname "$0")/.."

args=(
  --prefix=/usr
  -Dprofile=devel
  -Dcli=false
  -Dapp-id=io.github.yx14ng.rnote.Devel
  "-Dapp-name=Rnote (YX14ng)"
)
if [[ -d _mesonbuild ]]; then
  meson setup --reconfigure "${args[@]}" _mesonbuild
else
  meson setup "${args[@]}" _mesonbuild
fi
