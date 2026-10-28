#!/usr/bin/env bash
# Prüft, dass alle erwarteten Dateien am Entwurfs-Release hängen, hängt eine Prüfsummendatei an
# und veröffentlicht das Release als „Latest“. Bricht ohne Veröffentlichung ab, wenn Dateien fehlen.
# Aufruf: publish.sh <tag> <version>
set -euo pipefail

tag="${1:?Aufruf: publish.sh <tag> <version>}"
version="${2:?Aufruf: publish.sh <tag> <version>}"

d=$(mktemp -d)
trap 'rm -rf "$d"' EXIT

gh release view "$tag" --json assets --jq '.assets[].name' > "$d/names"
node scripts/release/expected-assets.mjs "$version" --check "$d/names"

gh release download "$tag" --dir "$d/files" --pattern '*'
(cd "$d/files" && sha256sum * > ../SHA256SUMS)
gh release upload "$tag" "$d/SHA256SUMS" --clobber

gh release edit "$tag" --draft=false --latest
echo "Release $tag veröffentlicht und als Latest markiert."
