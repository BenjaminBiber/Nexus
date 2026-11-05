#!/usr/bin/env bash
# Erhöht die Version im ganzen Repo, schreibt den Changelog-Abschnitt, committet und taggt.
# Aufruf: version-job.sh <patch|minor|major> <dry_run>
# Ausgaben „version“ und „tag“ gehen nach $GITHUB_OUTPUT; ohne die Variable auf stdout, damit sich
# das Skript lokal in einem Temp-Klon durchspielen lässt. Gepusht wird nur, wenn dry_run nicht „true“ ist.
set -euo pipefail

kind="${1:?Aufruf: version-job.sh <patch|minor|major> <dry_run>}"
dry="${2:-false}"
output="${GITHUB_OUTPUT:-/dev/stdout}"

version=$(node scripts/release/bump.mjs next "$kind")
tag="v$version"
echo "Nächste Version: $version (Tag $tag)"

# Netz für den seltenen Fall, dass seit dem Checkout schon jemand dieses Tag angelegt hat
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  echo "Abbruch: Tag $tag existiert schon." >&2
  exit 1
fi

# bump.mjs setzt alle Stellen (auch Cargo.lock und package-lock.json) und prüft danach selbst –
# deshalb braucht dieser Job weder cargo noch npm install
node scripts/release/bump.mjs set "$version" --changelog --date "$(date -u +%F)"

if git diff --quiet; then
  echo "Abbruch: $version steht schon überall, es gibt nichts zu committen." >&2
  exit 1
fi

git config user.name 'nexus-release[bot]'
git config user.email 'nexus-release@users.noreply.github.com'
git commit -am "Release $tag"
git tag -a "$tag" -m "Release $tag"

if [ "$dry" = 'true' ]; then
  echo "Probelauf: Commit und Tag $tag bleiben lokal, es wird nicht gepusht."
else
  # Atomar: Lehnt GitHub main ab (etwa beim Re-run eines alten Laufs), darf der Tag nicht allein landen.
  # RELEASE_REMOTE: wohin gepusht wird; der Gitea-Workflow setzt hier die Adresse des GitHub-Repos samt Token
  git push --atomic "${RELEASE_REMOTE:-origin}" HEAD:main "refs/tags/$tag"
fi

{
  echo "version=$version"
  echo "tag=$tag"
} >> "$output"
