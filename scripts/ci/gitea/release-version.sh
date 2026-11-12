#!/usr/bin/env bash
# Gitea-Release, Job „version“: Branch prüfen, Tags von GitHub holen, Version setzen, Commit und Tag nach
# GitHub pushen, Entwurfs-Release bei GitHub anlegen. Ausgaben version, tag und release_id gehen nach
# $GITHUB_OUTPUT (setzt der Runner; ohne die Variable auf stdout).
# Aufruf: release-version.sh <patch|minor|major> <dry_run> <ref>
set -euo pipefail
# shellcheck source=common.sh
source "$(dirname "$0")/common.sh"

bump="${1:?Aufruf: release-version.sh <patch|minor|major> <dry_run> <ref>}"
dry="${2:?Aufruf: release-version.sh <patch|minor|major> <dry_run> <ref>}"
ref="${3:?Aufruf: release-version.sh <patch|minor|major> <dry_run> <ref>}"
output="${GITHUB_OUTPUT:-/dev/stdout}"

scripts/release/check-branch.sh "$ref" "$dry"

# GitHub ist die Quelle der Tags: bump.mjs rechnet aus dem höchsten Tag „v*“, und ein Mirror kann hinterherhinken
if [ "$dry" = 'true' ] && [ -z "${RELEASE_GITHUB_TOKEN:-}" ]; then
  echo "Probelauf ohne RELEASE_GITHUB_TOKEN: es gelten die Tags des Gitea-Klons."
else
  git fetch --force --tags "$(github_url)"
fi

# version-job.sh schreibt seine Ausgaben erst in eine eigene Datei: der Tag wird hier noch gebraucht
own=$(mktemp)
trap 'rm -f "$own"' EXIT
# Gepusht wird nach GitHub; der Push von main scheitert, wenn GitHub einen anderen Stand hat als dieser Checkout
if [ "$dry" = 'true' ]; then remote=origin; else remote="$(github_url)"; fi
GITHUB_OUTPUT="$own" RELEASE_REMOTE="$remote" scripts/release/version-job.sh "$bump" "$dry"
cat "$own" >> "$output"

if [ "$dry" != 'true' ]; then
  # shellcheck disable=SC1090
  source "$own"
  GITHUB_OUTPUT="$output" scripts/release/draft-release.sh "$tag" "$version"
fi
