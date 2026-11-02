#!/usr/bin/env bash
# Prüft vor einem Release, ob der Lauf auf „main“ steht und die CI für genau diesen Commit grün ist.
# Aufruf: check-ci.sh <ref> <sha> <dry_run> [skip_ci]   (ref wie „refs/heads/main“, dry_run und skip_ci „true“ oder „false“)
# Im Probelauf wird nur gewarnt, damit sich der Workflow auch von einem Zweig aus durchspielen lässt.
# Mit skip_ci „true“ entfällt die Prüfung der CI; den Branch prüft das Skript trotzdem.
# shellcheck disable=SC1111  # „…“ in den Meldungen sind deutsche Anführungszeichen, keine Tippfehler
set -euo pipefail

ref="${1:?Aufruf: check-ci.sh <ref> <sha> <dry_run>}"
sha="${2:?Aufruf: check-ci.sh <ref> <sha> <dry_run>}"
dry="${3:-false}"
skip_ci="${4:-false}"

# Dateiname der CI; in .github/workflows/ci.yml steht derselbe Hinweis, dass er fest ist
CI_WORKFLOW='ci.yml'
RELEASE_BRANCH='refs/heads/main'

# Ein Grund zum Abbrechen – im Probelauf nur eine Warnung
problem() {
  if [ "$dry" = 'true' ]; then
    echo "Warnung (Probelauf, kein Abbruch): $1" >&2
  else
    echo "Abbruch: $1" >&2
    exit 1
  fi
}

if [ "$ref" != "$RELEASE_BRANCH" ]; then
  problem "Ein Release entsteht nur aus „$RELEASE_BRANCH“, gestartet wurde aber auf „$ref“."
else
  echo "Branch: $ref"
fi

if [ "$skip_ci" = 'true' ]; then
  echo "Warnung: Die CI für $sha wird auf Wunsch nicht geprüft (skip_ci)." >&2
  exit 0
fi

# Der jüngste CI-Lauf genau dieses Commits als eine Zeile „Status<Tab>Ergebnis<Tab>Adresse“; ohne Lauf leer.
# Scheitert die Abfrage selbst, ist das genauso ein Grund wie ein roter Lauf – im Probelauf also nur eine Warnung
errors=$(mktemp)
trap 'rm -f "$errors"' EXIT
if ! run=$(gh run list --workflow "$CI_WORKFLOW" --commit "$sha" --limit 1 \
  --json status,conclusion,url \
  --jq '.[0] // empty | [.status, (.conclusion // "offen"), .url] | @tsv' 2>"$errors"); then
  problem "Der Status der CI für $sha lässt sich nicht abfragen: $(tr '\n' ' ' <"$errors")"
  exit 0 # nur der Probelauf kommt hierhin; sonst hat „problem“ schon abgebrochen
fi

if [ -z "$run" ]; then
  problem "Für $sha gibt es keinen Lauf von $CI_WORKFLOW. Erst die CI abwarten."
  exit 0 # dito
fi

IFS=$'\t' read -r status conclusion url <<<"$run"

if [ "$status" != 'completed' ] || [ "$conclusion" != 'success' ]; then
  problem "CI für $sha ist nicht grün: Status „$status“, Ergebnis „$conclusion“ ($url)."
else
  echo "CI für $sha ist grün ($url)."
fi
