#!/usr/bin/env bash
# Gitea-Workflows, Job „test“: Oberfläche bauen, Typcheck, Tests, Vertragstests. Entspricht dem Job „test“
# in .github/workflows/ci.yml.
set -euo pipefail
# shellcheck source=common.sh
source "$(dirname "$0")/common.sh"

setup_git_for_tests
unoptimized_dependencies

npm ci
# Die Plugin-Tests bauen das Plugin und brauchen dafür die gebaute Oberfläche
npm run build
npm run typecheck
# Plugin-Tests und Vertrag brauchen das Binary; der Plugin-Build braucht das Release-Binary
cargo build -p nexus-cli --bin nexus
cargo build --release -p nexus-cli --bin nexus
npm test
if [ -f contract/run.mjs ]; then npm run contract; fi
