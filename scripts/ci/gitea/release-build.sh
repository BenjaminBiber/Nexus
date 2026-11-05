#!/usr/bin/env bash
# Gitea-Release, Job „build“: baut die Installer eines Ziels mit dem Tauri-CLI und hängt sie samt CLI-Archiv
# an den Entwurf bei GitHub. Ersetzt tauri-action aus .github/workflows/release.yml.
# Aufruf: release-build.sh <rust-ziel> <bundles> <dry_run>
#   RELEASE_VERSION und RELEASE_TAG kommen aus dem Job „version“.
set -euo pipefail
# shellcheck source=common.sh
source "$(dirname "$0")/common.sh"

target="${1:?Aufruf: release-build.sh <rust-ziel> <bundles> <dry_run>}"
bundles="${2:?Aufruf: release-build.sh <rust-ziel> <bundles> <dry_run>}"
dry="${3:?Aufruf: release-build.sh <rust-ziel> <bundles> <dry_run>}"
version="${RELEASE_VERSION:?RELEASE_VERSION fehlt – kommt aus dem Job „version“.}"
tag="${RELEASE_TAG:?RELEASE_TAG fehlt – kommt aus dem Job „version“.}"

# Ein nicht angelegtes Secret kommt als leerer Text an. Tauri signiert aber schon, sobald APPLE_CERTIFICATE
# gesetzt ist, auch leer; deshalb verschwinden leere Werte ganz
for name in APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY APPLE_API_KEY APPLE_API_ISSUER \
  APPLE_API_PRIVATE_KEY AZURE_SIGNING_ENDPOINT AZURE_SIGNING_ACCOUNT AZURE_SIGNING_PROFILE AZURE_CLIENT_ID \
  AZURE_CLIENT_SECRET AZURE_TENANT_ID; do
  if [ -z "${!name:-}" ]; then unset "$name"; fi
done

# Der Notarisierungs-Schlüssel muss als Datei vorliegen
if [ -n "${APPLE_API_PRIVATE_KEY:-}" ]; then
  key="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/AuthKey.p8"
  printf '%s' "$APPLE_API_PRIVATE_KEY" > "$key"
  export APPLE_API_KEY_PATH="$key"
  trap 'rm -f "$key"' EXIT
fi

if [ "$dry" = 'true' ]; then
  # Im Probelauf liegt das Tag nirgends: gebaut wird der gestartete Commit, mit der neuen Version nur lokal
  node scripts/release/bump.mjs set "$version"
else
  # Gebaut wird das Tag aus dem Job „version“, nicht der Stand des Branches
  git fetch --force --no-tags "$(github_url)" "refs/tags/$tag:refs/tags/$tag"
  git checkout --force --detach "$tag"
fi

rustup target add "$target"
# copy-server.mjs baut den Server mit --target wie die App; das CLI-Archiv findet ihn dann unter target/<ziel>/release
export NEXUS_RELEASE_TARGET="$target"

# signCommand ruft trusted-signing-cli auf; nur nötig, wenn die Azure-Signatur eingerichtet ist
if is_windows && [ -n "${AZURE_SIGNING_ENDPOINT:-}" ] && ! command -v trusted-signing-cli > /dev/null; then
  cargo install trusted-signing-cli --version 0.11.0 --locked
fi

npm ci
# Ohne TAURI_SIGNING_PRIVATE_KEY bricht der Build ab: tauri.release.conf.json schaltet die Updater-Dateien ein
(cd apps/desktop && npx tauri build --target "$target" --bundles "$bundles" --config src-tauri/tauri.release.conf.json)

node scripts/release/cli-archive.mjs --version "$version" --target "$target" > /dev/null

args=(--version "$version" --target "$target" --tag "$tag")
if [ "$dry" = 'true' ]; then args+=(--dry-run); fi
node scripts/release/upload-bundles.mjs "${args[@]}"
