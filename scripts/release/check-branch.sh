#!/usr/bin/env bash
# Prüft vor einem Release, ob der Lauf auf „main“ steht. Die CI prüft das Skript nicht mehr: der
# Release-Workflow startet sie selbst (Job „ci“) parallel zu den Builds, und „publish“ wartet auf sie.
# Aufruf: check-branch.sh <ref> <dry_run>   (ref wie „refs/heads/main“, dry_run „true“ oder „false“)
# Im Probelauf wird nur gewarnt, damit sich der Workflow auch von einem Zweig aus durchspielen lässt.
# shellcheck disable=SC1111  # „…“ in den Meldungen sind deutsche Anführungszeichen, keine Tippfehler
set -euo pipefail

ref="${1:?Aufruf: check-branch.sh <ref> <dry_run>}"
dry="${2:-false}"

RELEASE_BRANCH='refs/heads/main'

if [ "$ref" = "$RELEASE_BRANCH" ]; then
  echo "Branch: $ref"
elif [ "$dry" = 'true' ]; then
  echo "Warnung (Probelauf, kein Abbruch): Ein Release entsteht nur aus „$RELEASE_BRANCH“, gestartet wurde aber auf „$ref“." >&2
else
  echo "Abbruch: Ein Release entsteht nur aus „$RELEASE_BRANCH“, gestartet wurde aber auf „$ref“." >&2
  exit 1
fi
