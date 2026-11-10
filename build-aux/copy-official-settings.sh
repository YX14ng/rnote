#!/usr/bin/env bash
# Copies the settings of Flathub's Rnote (document preset, pens, colors...) to the fork's
# Flatpak, for every key both have. Reads the official app's settings, never changes them.
set -euo pipefail

official=com.github.flxzt.rnote
fork=io.github.yx14ng.rnote

mapfile -t fork_keys < <(flatpak run --command=gsettings "$fork" list-keys "$fork")
copied=0
while read -r _schema key value; do
  [[ "$key" == library-* ]] && continue
  for fork_key in "${fork_keys[@]}"; do
    if [[ "$fork_key" == "$key" ]]; then
      flatpak run --command=gsettings "$fork" set "$fork" "$key" "$value"
      copied=$((copied + 1))
      break
    fi
  done
done < <(flatpak run --command=gsettings "$official" list-recursively "$official")
echo "Copied $copied settings from $official to $fork."
