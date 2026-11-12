#!/usr/bin/env bash
# Gitea-Release, Job „publish“: latest.json aus den Signaturen am Entwurf bauen und anhängen, dann wie in
# GitHub Actions prüfen, Prüfsummen anhängen und veröffentlichen (publish.sh).
#   RELEASE_VERSION und RELEASE_TAG kommen aus dem Job „version“; gh braucht GH_TOKEN und GH_REPO.
set -euo pipefail
# shellcheck source=common.sh
source "$(dirname "$0")/common.sh"

version="${RELEASE_VERSION:?RELEASE_VERSION fehlt – kommt aus dem Job „version“.}"
tag="${RELEASE_TAG:?RELEASE_TAG fehlt – kommt aus dem Job „version“.}"

d=$(mktemp -d)
trap 'rm -rf "$d"' EXIT

# publish.sh und updater-json.mjs lesen Changelog und erwartete Dateien aus dem Stand des Tags
git fetch --force --no-tags "$(github_url)" "refs/tags/$tag:refs/tags/$tag"
git checkout --force --detach "$tag"

gh release download "$tag" --dir "$d/sigs" --pattern '*.sig'
node scripts/release/updater-json.mjs --version "$version" --tag "$tag" --repo "$RELEASE_GITHUB_REPO" --sigs "$d/sigs" --out "$d/latest.json"
gh release upload "$tag" "$d/latest.json" --clobber

scripts/release/publish.sh "$tag" "$version"
