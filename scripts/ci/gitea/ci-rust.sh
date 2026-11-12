#!/usr/bin/env bash
# Gitea-Workflows, Job „rust“: fmt, clippy und Tests des Cargo-Workspace. Entspricht dem Job „rust“
# in .github/workflows/ci.yml.
set -euo pipefail
# shellcheck source=common.sh
source "$(dirname "$0")/common.sh"

require_tool cargo "Rust über https://rustup.rs installieren."
setup_git_for_tests
unoptimized_dependencies
# Ohne Debug-Info: die Testbinaries des ganzen Workspace samt Tauri füllen sonst die Platte
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0

cargo fmt --all --check

# Das Build-Skript der Desktop-App verlangt Sidecar, gebaute Oberfläche und Plugin-Bündel;
# für Lint und Tests reichen leere Platzhalter
ziel="$(rustc -vV | sed -n 's/^host: //p')"
endung=""
if is_windows; then endung=".exe"; fi
mkdir -p apps/desktop/src-tauri/binaries packages/web/dist packages/plugin/dist/nexus
touch "apps/desktop/src-tauri/binaries/nexus-$ziel$endung" packages/web/dist/index.html packages/plugin/dist/nexus/.platzhalter

cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --no-fail-fast

# Die TypeScript-Typen aus ts-rs sind eingecheckt: neu erzeugen und prüfen, dass sich nichts ändert.
# Reicht auf einer Plattform, weil das Ergebnis überall gleich ist
if [ "$(uname -s)" = "Linux" ]; then
  cargo test -p nexus-core export_bindings
  git diff --exit-code -- packages/web/src/api/generated
  neu="$(git status --porcelain -- packages/web/src/api/generated)"
  if [ -n "$neu" ]; then
    echo "Nicht eingecheckte erzeugte Typen:"
    echo "$neu"
    exit 1
  fi
fi
