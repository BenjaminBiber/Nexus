#!/usr/bin/env bash
# Richtet den Linux-Container eines Gitea-Runners (Label nexus-linux, Image catthehacker/ubuntu:act-22.04)
# für Nexus ein. Node kommt aus actions/setup-node im Workflow.
# Aufruf: linux-setup.sh <gh|full>
#   gh    nur das GitHub-CLI (Jobs „version“ und „publish“)
#   full  dazu die Systempakete von Tauri und Rust (Tests und Builds)
# Ubuntu 22.04 wie bisher in GitHub Actions: das AppImage läuft dann auch auf älteren Distributionen.
set -euo pipefail

mode="${1:?Aufruf: linux-setup.sh <gh|full>}"

# Das act-Image läuft als root; auf einem Rechner mit normalem Benutzer geht es über sudo
sudo=""
if [ "$(id -u)" != "0" ]; then sudo="sudo"; fi
export DEBIAN_FRONTEND=noninteractive

if ! command -v gh > /dev/null; then
  $sudo apt-get update -qq
  $sudo apt-get install -y -qq curl ca-certificates > /dev/null
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | $sudo tee /usr/share/keyrings/githubcli-archive-keyring.gpg > /dev/null
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    | $sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
  $sudo apt-get update -qq
  $sudo apt-get install -y -qq gh > /dev/null
fi

if [ "$mode" = "gh" ]; then exit 0; fi

$sudo apt-get update -qq
# Pakete wie in .github/workflows; wget, file und xdg-utils hat das Image von GitHub ab Werk, Tauri braucht sie für das AppImage
$sudo apt-get install -y -qq git build-essential pkg-config curl wget file xdg-utils \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev patchelf rpm > /dev/null

# rustup installiert die Toolchain aus rust-toolchain.toml beim ersten Aufruf selbst
if ! command -v rustup > /dev/null && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none > /dev/null
fi
export PATH="$HOME/.cargo/bin:$PATH"
rustup show active-toolchain || rustup toolchain install
# Für die folgenden Schritte des Jobs
if [ -n "${GITHUB_PATH:-}" ]; then echo "$HOME/.cargo/bin" >> "$GITHUB_PATH"; fi
