#!/usr/bin/env bash
# Legt den Entwurf des Releases zum Tag an: Text aus dem Changelog-Abschnitt, dazu die erzeugten Notizen.
# Aufruf: draft-release.sh <tag> <version>
# Ausgabe „release_id“ geht nach $GITHUB_OUTPUT; ohne die Variable auf stdout.
set -euo pipefail

tag="${1:?Aufruf: draft-release.sh <tag> <version>}"
version="${2:?Aufruf: draft-release.sh <tag> <version>}"
output="${GITHUB_OUTPUT:-/dev/stdout}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY fehlt – das Skript läuft im Release-Workflow.}"

# Wie „gh release create --verify-tag“: ohne den Tag bei GitHub würde die API ihn sonst selbst anlegen
if ! gh api "repos/$repo/git/ref/tags/$tag" --silent; then
  echo "Abbruch: Der Tag $tag liegt nicht in $repo." >&2
  exit 1
fi

# Der Abschnitt des Changelogs kommt in eine Temp-Datei, damit der Arbeitsordner sauber bleibt
notes=$(mktemp)
trap 'rm -f "$notes"' EXIT
node scripts/release/bump.mjs notes "$version" > "$notes"

# Die Id kommt direkt aus der Antwort: Die Liste der Releases zeigt einen frischen Entwurf erst nach
# einer Weile, und GET /releases/tags/<tag> findet gar keine Entwürfe.
# GitHub stellt „body“ vor die erzeugten Notizen.
id=$(gh api -X POST "repos/$repo/releases" \
  -f tag_name="$tag" -f name="Nexus $tag" -F body=@"$notes" \
  -F draft=true -F generate_release_notes=true --jq .id)
if [ -z "$id" ]; then
  echo "Abbruch: GitHub hat für den Entwurf zu $tag keine Id geliefert." >&2
  exit 1
fi
echo "Entwurfs-Release $id für $tag angelegt."

echo "release_id=$id" >> "$output"
