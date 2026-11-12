# CI und Release über Gitea Actions

`.gitea/workflows/ci.yml` und `release.yml` machen dasselbe wie die Dateien unter `.github/workflows`, laufen
aber auf **eigenen Runnern** (act_runner). Gitea liest `.gitea/workflows` und übergeht dann `.github/workflows`;
GitHub wiederum kennt `.gitea/` nicht. Beide Varianten stören sich also nicht.

Das Release landet weiter bei **GitHub**: Versions-Commit und Tag gehen auf GitHubs `main`, Installer,
CLI-Archive, `latest.json` und `SHA256SUMS` an das GitHub-Release. Dort sucht der Updater der App
(`tauri.conf.json` → `plugins.updater.endpoints`).

## Ablauf des Releases

| Job | Runner-Label | Was er tut |
| --- | --- | --- |
| `version` | `nexus-linux` | Branch prüfen, Tags von GitHub holen, Version setzen, Commit + Tag nach GitHub pushen, Entwurf anlegen |
| `build` (macOS) | `nexus-macos` | `tauri build` für `aarch64-apple-darwin`, Upload von dmg, App-Archiv, Signatur, CLI-Archiv |
| `build` (Windows) | `nexus-windows` | dasselbe für `x86_64-pc-windows-msvc` (NSIS) |
| `build` (Linux) | `nexus-linux` | dasselbe für AppImage, deb, rpm |
| `ci_test`, `ci_rust` | alle drei | Tests, Typcheck, fmt, clippy; parallel zu den Builds |
| `publish` | `nexus-linux` | `latest.json` aus den `.sig`-Dateien bauen, Dateien prüfen, Prüfsummen, veröffentlichen |

`publish` läuft nur, wenn alle Builds und die CI grün sind. Sonst bleibt das Release ein Entwurf; Tag und
Versions-Commit liegen dann aber schon bei GitHub (wie in GitHub Actions). Der nächste Lauf nimmt die nächste
Version, oder Tag und Entwurf werden von Hand gelöscht.

Was in GitHub Actions `tauri-action` erledigt, übernehmen hier `scripts/release/upload-bundles.mjs` (Installer
unter den erwarteten Namen an das Release) und `scripts/release/updater-json.mjs` (`latest.json`).
`bench` gibt es hier nicht: seine Ausgangswerte gelten nur für die Runner von GitHub.

## Starten

Im Gitea-Repo **Actions → Release → Run workflow** auf `main` (nur CI: **Actions → CI**):

| Eingabe | Werte | Bedeutung |
| --- | --- | --- |
| `bump` | `patch` / `minor` / `major` | Art der Änderungen |
| `dry_run` | `false` / `true` | Probelauf: nichts wird gepusht oder hochgeladen, gebaut wird trotzdem |
| `skip_ci` | `false` / `true` | CI weglassen |

Pushes starten keinen Workflow. Immer nur ein Release zur Zeit starten.

## Einrichtung

### 1. Repo in Gitea

Der Versions-Commit wird auf GitHubs `main` gepusht und scheitert, wenn dort ein anderer Stand liegt als in
Gitea. Gitea muss also den Stand von GitHub haben:

- **Pull-Mirror von GitHub** (empfohlen): vor dem Release „Jetzt synchronisieren“. Falls deine Gitea-Version
  für Mirror-Repos keine Actions anbietet, ein normales Repo nehmen und vor dem Release `git push gitea main`.
- **Kein Push-Mirror von Gitea nach GitHub:** der überschreibt GitHub mit `--mirror` und löscht dabei
  Versions-Commit und Tags, die nur bei GitHub liegen.

Actions müssen für das Repo eingeschaltet sein (Einstellungen → Repository → Actions).

### 2. Secrets und Variablen

Im Gitea-Repo unter Einstellungen → Actions → Secrets bzw. Variables. Namen dürfen in Gitea nicht mit
`GITHUB_` oder `GITEA_` beginnen.

| Name | Art | Pflicht | Inhalt |
| --- | --- | --- | --- |
| `RELEASE_GITHUB_TOKEN` | Secret | ja | Fine-grained Token von GitHub, nur Repo `Nexus`: **Contents: Read and write** |
| `TAURI_SIGNING_PRIVATE_KEY` | Secret | ja | privater Schlüssel des Updaters (wie das GitHub-Secret) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Secret | ja | sein Passwort |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | Secret | nein | macOS-Signatur; ohne bleibt der Build unsigniert |
| `APPLE_API_PRIVATE_KEY`, `APPLE_API_KEY`, `APPLE_API_ISSUER` | Secret | nein | Notarisierung; `APPLE_API_PRIVATE_KEY` ist der Inhalt der `.p8`-Datei |
| `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT`, `AZURE_SIGNING_PROFILE` | Variable | nein | Windows-Signatur über Azure Trusted Signing |
| `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` | Secret | nein | dazu |

Das Ziel bei GitHub (`BenjaminBiber/Nexus`) steht in `release.yml` unter `env`.

### 3. Runner

Jeder Job verlangt eines von drei Labels. act_runner registrieren (`act_runner register`) und die Labels in
`config.yaml` unter `runner.labels` setzen:

| Rechner | Label | Modus |
| --- | --- | --- |
| Mac (Apple Silicon) | `nexus-macos:host` | direkt auf dem Rechner |
| Windows (x64) | `nexus-windows:host` | direkt auf dem Rechner |
| Rechner mit Docker, x64 | `nexus-linux:docker://catthehacker/ubuntu:act-22.04` | im Container |

Für `nexus-linux` eignet sich der Windows-Rechner mit Docker Desktop: ein act_runner kann beide Labels tragen
(`nexus-windows:host` und `nexus-linux:docker://…`). Dann `runner.capacity` auf 2 setzen, sonst laufen alle
Jobs nacheinander. Auf dem Mac würde der x64-Container emuliert und wäre sehr langsam. Ohne Runner mit
passendem Label bleiben die Jobs in der Warteschlange.

Der Runner mit dem Label `nexus-linux` braucht in seiner `config.yaml` einen Init-Prozess für die Job-Container,
danach den Runner neu starten:

```yaml
container:
  options: --init
```

Ohne ihn ist PID 1 im Container das `sleep`, mit dem der Runner den Container am Leben hält. Das räumt beendete
verwaiste Prozesse nie ab, sie bleiben als Zombies stehen, und die Tests, die das Beenden von Enkelprozessen prüfen,
scheitern (siehe „Häufige Fehler“).

Auf den Rechnern im Host-Modus muss vorhanden sein:
- **Git**, **Node** (act_runner braucht es für die Actions; die Version 26 holt `actions/setup-node`),
  **rustup** (die Toolchain aus `rust-toolchain.toml` lädt rustup selbst)
- **GitHub-CLI `gh`** (für den Upload an das Release)
- Mac: Xcode Command Line Tools
- Windows: **Git for Windows** unter `C:\Program Files\Git` (die Jobs rufen ausdrücklich dessen Bash auf, nicht
  ein `bash` aus dem PATH, das auch das von WSL sein kann), Visual Studio Build Tools mit „Desktopentwicklung
  mit C++“. NSIS lädt Tauri selbst.

Im Linux-Container richtet `linux-setup.sh` bei jedem Lauf Systempakete, Rust und `gh` ein.

Die Jobs schreiben nicht in die globale Git-Konfiguration des Rechners (`GIT_CONFIG_GLOBAL` zeigt auf eine
Datei im Temp-Ordner des Laufs). Die Test-Jobs checken mit `core.autocrlf=false` aus (`GIT_CONFIG_*` unter `env`
des Jobs), auch wenn der Rechner anderes einstellt.

**Bauzeit:** act_runner legt je Job einen frischen Arbeitsordner an, und `rust-cache` aus den GitHub-Workflows
fehlt hier. Rust baut also jedes Mal von vorn; deshalb sind die Zeitlimits auf 90 Minuten gesetzt.

## Häufige Fehler

**Checkout im Linux-Container: `server certificate verification failed` oder `unable to get local issuer
certificate`.** Der Gitea-Server schickt nur sein eigenes Zertifikat ohne das Zwischenzertifikat der
Zertifizierungsstelle. Browser, Windows und macOS laden es selbst nach, `git` und `curl` unter Linux nicht. Abhilfe am
Server: das Zertifikat mit Kette ausliefern, also eigenes Zertifikat und Zwischenzertifikat in einer Datei
(„fullchain“). Bei Nginx Proxy Manager gehört das Zwischenzertifikat beim eigenen Zertifikat in das Feld
„Intermediate Certificate“. Prüfen, ob der Server jetzt beide schickt (Ausgabe `2`):

```bash
echo | openssl s_client -connect <gitea-host>:443 -servername <gitea-host> -showcerts 2>/dev/null | grep -c 'BEGIN CERTIFICATE'
```

**Linux: `beendet beim Stoppen auch Enkelprozesse …` (`ServerHandle.test.mjs`) oder
`process_group::tests::beendet_auch_das_enkelkind` schlagen fehl.** Der beendete Enkelprozess bleibt als Zombie
stehen, weil im Job-Container kein Init-Prozess läuft; `kill(pid, 0)` meldet ihn dann weiter als vorhanden. Abhilfe:
`container.options: --init` in der `config.yaml` des Runners (siehe Abschnitt 3), danach den Runner neu starten.
Prüfen, während ein Job läuft (Ausgabe `true`):

```bash
docker inspect --format '{{.HostConfig.Init}}' $(docker ps -q --filter name=GITEA-ACTIONS | head -n 1)
```

**Windows: Tests erwarten `\n` und bekommen `\r\n`, oder `SyntaxError: Invalid or unexpected token` in
`HelpScenarios.test.mjs`.** Der Checkout hat die Zeilenenden auf CRLF umgestellt: Git for Windows setzt
`core.autocrlf=true` in seiner Systemkonfiguration. Vitest scheitert an einer Shebang-Zeile mit `\r`. Die Test-Jobs
setzen deshalb `core.autocrlf=false` über `GIT_CONFIG_*` unter `env`; fehlt das, die Workflows mit
`.github/workflows/ci.yml` vergleichen. Prüfen im Arbeitsordner des Jobs (`w/crlf` heißt umgestellt):

```bash
git ls-files --eol scripts/release/tests/fixtures/repo/CHANGELOG.md
```

**`cargo: command not found` auf dem Mac.** act_runner läuft als launchd-Dienst mit knappem PATH, ohne die
Ergänzungen aus `.zshrc`. `common.sh` nimmt deshalb `~/.cargo/bin`, `/opt/homebrew/bin` und `/usr/local/bin` dazu.
Liegt Rust woanders, den PATH des Dienstes ergänzen (im plist unter `EnvironmentVariables`).
