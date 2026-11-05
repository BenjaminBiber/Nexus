# Gemeinsames für die Job-Skripte der Gitea-Workflows (wird mit `source` geladen, nicht ausgeführt).
# Läuft unter bash auf macOS (auch bash 3.2), Linux und Git Bash unter Windows.
# shellcheck shell=bash

is_windows() {
  case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) return 0 ;; *) return 1 ;; esac
}

# Ein Runner als Dienst (launchd auf dem Mac, systemd) startet mit knappem PATH, ohne die Ergänzungen der
# Login-Shell: cargo aus rustup und gh aus Homebrew fehlen dann. Die üblichen Orte kommen vorn dazu, soweit
# es sie gibt. Unter Windows nicht: dort stimmt der PATH, und ein CARGO_HOME wie C:\… zerlegte ihn am Doppelpunkt
add_tool_paths() {
  if is_windows; then return 0; fi
  local dir
  for dir in /usr/local/bin /opt/homebrew/bin "${CARGO_HOME:-$HOME/.cargo}/bin"; do
    if [ -d "$dir" ]; then
      case ":$PATH:" in
        *":$dir:"*) ;;
        *) PATH="$dir:$PATH" ;;
      esac
    fi
  done
  export PATH
}
add_tool_paths

# Bricht mit Hinweis ab, wenn ein Programm auf dem Runner-Rechner fehlt
require_tool() {
  if ! command -v "$1" > /dev/null; then
    echo "$1 fehlt auf dem Runner-Rechner (PATH: $PATH). $2" >&2
    exit 1
  fi
}

# Git-Einstellungen der Tests landen in einer Datei des Laufs statt in der globalen Konfiguration
# des Runner-Rechners. Die Tests legen eigene Git-Projekte an; autocrlf false hält Vergleichsdateien
# unter Windows bei \n.
setup_git_for_tests() {
  export GIT_CONFIG_GLOBAL="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/nexus-ci.gitconfig"
  git config --global user.name "CI"
  git config --global user.email "ci@example.invalid"
  git config --global init.defaultBranch main
  git config --global core.autocrlf false
  if is_windows; then git config --global core.longpaths true; fi
}

# Adresse des GitHub-Repos mit Token: dorthin gehen Versions-Commit und Tag, von dort holen die Builds das Tag.
# Die Namen beginnen nicht mit GITHUB_: solche Variablen setzt der Runner selbst (in Gitea mit den Werten
# des Gitea-Repos), und Gitea lässt sie für Secrets und Variablen nicht zu
github_url() {
  : "${RELEASE_GITHUB_TOKEN:?RELEASE_GITHUB_TOKEN fehlt (Secret in Gitea).}"
  : "${RELEASE_GITHUB_REPO:?RELEASE_GITHUB_REPO fehlt (owner/name des GitHub-Repos).}"
  echo "https://x-access-token:${RELEASE_GITHUB_TOKEN}@github.com/${RELEASE_GITHUB_REPO}.git"
}

# Fremde Crates in der CI unoptimiert übersetzen: Cargo.toml setzt für `tauri dev` opt-level 2, das kostet
# je Lauf viele Minuten, und Tests brauchen es nicht. Der nächste Checkout setzt die Datei zurück
unoptimized_dependencies() {
  printf '\n[profile.dev.package."*"]\nopt-level = 0\n' >> .cargo/config.toml
}
