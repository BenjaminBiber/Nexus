#!/usr/bin/env bash
# Legt den Entwurf des Releases zum Tag an: Text aus dem Changelog-Abschnitt, dazu die erzeugten Notizen.
# Aufruf: draft-release.sh <tag> <version>
# Ausgabe „release_id“ geht nach $GITHUB_OUTPUT; ohne die Variable auf stdout.
set -euo pipefail

tag="${1:?Aufruf: draft-release.sh <tag> <version>}"
version="${2:?Aufruf: draft-release.sh <tag> <version>}"
output="${GITHUB_OUTPUT:-/dev/stdout}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY fehlt – das Skript läuft im Release-Workflow.}"

# Der Abschnitt des Changelogs kommt in eine Temp-Datei, damit der Arbeitsordner sauber bleibt
notes=$(mktemp)
trap 'rm -f "$notes"' EXIT
node scripts/release/bump.mjs notes "$version" > "$notes"

gh release create "$tag" --draft --verify-tag --title "Nexus $tag" --notes-file "$notes" --generate-notes

# GET /releases/tags/<tag> findet keine Entwürfe – darum die Liste durchsuchen
id=$(gh api "repos/$repo/releases" --jq "[.[] | select(.tag_name == \"$tag\") | .id] | first // empty")
if [ -z "$id" ]; then
  echo "Abbruch: Das Entwurfs-Release zu $tag steht nicht in der Liste von $repo." >&2
  exit 1
fi
echo "Entwurfs-Release $id für $tag angelegt."

echo "release_id=$id" >> "$output"
