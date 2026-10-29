# Versionsskript (eine Version für das ganze Repo)

Ein Release soll **eine** Versionsquelle haben. Alle anderen Stellen – Rust-Crates, `Cargo.lock`, die
`package.json`-Dateien der npm-Workspaces, `package-lock.json`, das Plugin-Manifest und der Marktplatz-Eintrag –
ziehen in einem Schritt mit. Dieses Skript berechnet die nächste Version aus dem letzten Tag, setzt alle Stellen,
prüft Gleichheit und benennt den Changelog-Abschnitt „Unveröffentlicht“ in einen Versionsabschnitt um. Der
Release-Workflow ruft es auf; es braucht keine zusätzlichen Abhängigkeiten.

## Aufruf

```sh
node scripts/release/bump.mjs check                                   # Code 1 bei Abweichung
node scripts/release/bump.mjs next patch                              # gibt X.Y.Z aus
node scripts/release/bump.mjs set 0.2.0 --changelog --date 2026-11-01
node scripts/release/bump.mjs notes 0.2.0                             # Abschnitt aus CHANGELOG.md
```

```
node scripts/release/bump.mjs next <major|minor|patch> [--root <dir>]
node scripts/release/bump.mjs set <X.Y.Z> [--changelog] [--date YYYY-MM-DD] [--root <dir>]
node scripts/release/bump.mjs check [--root <dir>]
node scripts/release/bump.mjs notes <X.Y.Z> [--root <dir>]
node scripts/release/bump.mjs current [--root <dir>]
```

| Befehl | Bedeutung |
| --- | --- |
| `next <stelle>` | nächste Version: höchstes Tag `v*` um eine Stelle erhöht; ohne Tag die Version der Quelle |
| `set <X.Y.Z>` | setzt alle Stellen und prüft danach selbst (`check`); Code 1, wenn danach etwas abweicht |
| `check` | Code 0, wenn alle Stellen gleich sind, sonst Code 1 und eine Liste der Abweichungen |
| `notes <X.Y.Z>` | gibt den Abschnitt dieser Version aus `CHANGELOG.md` aus (ohne Überschrift) |
| `current` | gibt die Version der Quelle aus |

| Argument | Bedeutung |
| --- | --- |
| `--changelog` | benennt `## Unveröffentlicht` in `## X.Y.Z – <datum>` um und setzt einen leeren Abschnitt darüber (nur bei `set`) |
| `--date <datum>` | Datum der Überschrift (`YYYY-MM-DD`, Standard: heute, örtliche Zeit) |
| `--root <dir>` | Wurzel des Repos; Standard ist das Repo, in dem dieses Skript liegt – der Aufruf klappt also aus jedem Ordner |
| `--help` | zeigt die Möglichkeiten |

`--check` wird wie `check` gelesen; Release-Skripte schreiben ihn oft so.

Ergebnisse gehen auf stdout, Hinweise und Warnungen auf stderr. `next` und `current` geben deshalb nur die Version
aus und lassen sich direkt in eine Variable übernehmen (`VERSION=$(node scripts/release/bump.mjs next patch)`).

## Versionsstellen

Quelle ist `[workspace.package] version` der Workspace-`Cargo.toml`. Solange es die nicht gibt, ist es die
`[package] version` des einzigen Crates (heute `apps/desktop/src-tauri/Cargo.toml`); kommt die Workspace-Datei
dazu, zieht die Quelle von selbst dorthin um. Gibt es ohne Workspace-Datei mehrere Crates mit eigener Version,
bricht das Skript ab, statt zu raten.

| Datei | Stelle | Regel |
| --- | --- | --- |
| `Cargo.toml` (Workspace) | `[workspace.package] version` | **Quelle**, falls vorhanden |
| `<crate>/Cargo.toml` | `[package] version` | wird gesetzt; mit `version.workspace = true` bleibt die Datei unberührt |
| `Cargo.lock` | `[[package]]` der eigenen Crates → `version` | nur Einträge der Crates des Workspace, nie die der Abhängigkeiten |
| `<app>/src-tauri/tauri.conf.json` | `version` | **darf dort nicht stehen** (Tauri liest sie aus dem Crate): wird mit einer Warnung entfernt und gilt bei `check` als Abweichung |
| `packages/*/package.json`, `apps/*/package.json` | `version` | wird gesetzt; ein Paket ohne `version` bekommt keine |
| dieselben Dateien | exakt gepinnte Geschwister (`"@harness/core": "0.1.0"`) | werden mitgezogen; `^`, `~`, `*` und `workspace:*` bleiben unberührt |
| `package-lock.json` | `packages["<paket>"].version` und dieselben Pins | wird gesetzt; die Wurzel (`packages[""]`) hat keine Version |
| `packages/plugin/manifest/.claude-plugin/plugin.json` | `version` | wird gesetzt |
| `.claude-plugin/marketplace.json` | `plugins[<name des plugins>].version` | wird gesetzt, und angelegt, falls sie fehlt |

Die Workspace-Pakete kommen aus `workspaces` der Wurzel-`package.json`, die Crates aus `[workspace] members` und
zusätzlich aus `apps/*/src-tauri`. Gesucht wird also nur dort, wo das Repo selbst seine Mitglieder nennt – ein
`Cargo.toml` oder `package.json` in einem Testordner gerät dem Skript nicht in die Hände.

**Mitgezogene Pins.** Die Pakete pinnen sich untereinander exakt (`"@harness/core": "0.1.0"`). Würden nur die
`version`-Felder wandern, fände `npm ci` die alte Version nicht mehr. Deshalb sind diese Angaben eigene
Versionsstellen – in `package.json` und in `package-lock.json`.

## Formatierung der Dateien

Geändert wird nur die Stelle selbst, nie die ganze Datei:

- **TOML** zeilenweise: Abschnitt merken (`[workspace.package]`, `[package]`, `[[package]]`), in der richtigen
  Zeile nur die Zeichenkette tauschen. Kommentare hinter dem Wert, Reihenfolge und Leerzeilen bleiben. Die
  Formatversion oben in `Cargo.lock` (`version = 4`) steht in keinem Abschnitt und wird nie angefasst.
- **JSON** über die Textstelle des Werts, nicht über `JSON.parse` + `JSON.stringify`: ein neu geschriebenes JSON
  würde einzeilige Stellen wie `"author": { "name": "User" }` auseinanderziehen. So zeigt der Diff eines Releases
  nur die Versionszeilen, und ein zweites `set` auf dieselbe Version ändert keine Datei mehr.

Dateien, in denen sich nichts ändert, werden nicht geschrieben.

## Changelog

`--changelog` benennt `## Unveröffentlicht` in `## X.Y.Z – YYYY-MM-DD` um (Gedankenstrich U+2013) und setzt darüber
einen neuen leeren Abschnitt:

```md
## Unveröffentlicht

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges
```

Fehlt `## Unveröffentlicht`, bricht das Skript ab – dann wäre nicht klar, was in das Release gehört. Steht der
Abschnitt der Version schon da, bricht es ebenfalls ab. `notes <X.Y.Z>` gibt den Text zwischen `## X.Y.Z …` und
der nächsten `## `-Überschrift aus.

## Tags

`next` liest `git tag --list "v*"` im Wurzelordner und nimmt das höchste Tag in SemVer-Ordnung – `v0.10.0` steht
über `v0.9.0`. Vorabversionen (`v0.10.0-rc.1`) zählen nicht als Release; ein Tag, das keine Version ist, wird mit
einer Warnung übersprungen. Ohne Tag und ohne Git-Repo rechnet `next` ab der Version der Quelle. Eine
Vorabversion gilt dabei als Weg zu ihrer eigenen Version: `1.4.2-rc.1` + `patch` ist `1.4.2`.

## Release-Workflow

`.github/workflows/release.yml` wird von Hand gestartet (`workflow_dispatch`) und nimmt zwei Eingaben: `bump`
(`patch`, `minor` oder `major`) und `dry_run` (Probelauf). `concurrency: release` lässt nur ein Release zur Zeit zu.
Der erste Job `version` läuft in der Umgebung `release` – dort liegt `RELEASE_TOKEN`, mit dem der Checkout
arbeitet; ohne dieses Token lösen Commit und Tag keine weiteren Workflows aus. Er liefert die Ausgaben `version`,
`tag` und `release_id` für den Job `build` (unten) und den Job, der den Entwurf veröffentlicht (P6-5).

Die Logik steckt in drei Skripten, nicht im Workflow:

| Skript | Aufgabe |
| --- | --- |
| `check-ci.sh <ref> <sha> <dry_run>` | bricht ab, wenn der Lauf nicht auf `main` steht oder der jüngste Lauf von `ci.yml` für genau diesen Commit nicht `completed`/`success` ist; im Probelauf nur eine Warnung |
| `version-job.sh <stelle> <dry_run>` | `bump.mjs next` → `bump.mjs set --changelog`, Commit „Release vX.Y.Z“, Tag `vX.Y.Z`, Push nach `main` (im Probelauf kein Push); schreibt `version` und `tag` |
| `draft-release.sh <tag> <version>` | legt mit `gh release create --draft --verify-tag` den Entwurf an, Text aus `bump.mjs notes` plus erzeugte Notizen; schreibt `release_id` |

`cargo` und `npm install` braucht der Job nicht: `bump.mjs set` pflegt `Cargo.lock` und `package-lock.json` selbst.
Die Ausgaben gehen nach `$GITHUB_OUTPUT`; fehlt die Variable, nach stdout. Den Entwurf sucht `draft-release.sh`
über die Liste aller Releases, denn `GET /releases/tags/<tag>` findet keine Entwürfe.

### Probelauf auf dem eigenen Rechner

Nur in einem Temp-Klon, damit weder das Repo noch `origin` etwas abbekommt:

```sh
T=$(mktemp -d); git clone -q . "$T/r"; cd "$T/r"
GITHUB_OUTPUT=$T/out bash scripts/release/version-job.sh patch true
cat "$T/out"; git log -1 --oneline; git tag --list 'v*'; node scripts/release/bump.mjs check
```

`check-ci.sh` und `draft-release.sh` brauchen `gh` samt Token und ein Repo bei GitHub; lokal werden sie nur mit
`shellcheck` geprüft. Den Workflow selbst prüft `actionlint .github/workflows/release.yml`.

## Job `build`: Installer und CLI-Archiv

Der zweite Job `build` hängt am Tag aus `version` und baut je Plattform in einer Matrix:

| Runner | Ziel | Pakete |
| --- | --- | --- |
| `macos-14` | `aarch64-apple-darwin` | `app`, `dmg` |
| `macos-15-intel` | `x86_64-apple-darwin` | `app`, `dmg` |
| `windows-latest` | `x86_64-pc-windows-msvc` | `nsis` |
| `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `appimage`, `deb`, `rpm` |

Jeder Eintrag checkt das Tag aus, richtet Rust (Version wie in `rust-toolchain.toml`) und Node 26 ein, installiert
unter Linux die Systempakete von Tauri samt `rpm`, baut dann Oberfläche (`npm run build`) und das Binary
(`cargo build --release -p nexus-cli --bin nexus --target <ziel>`) und ruft `tauri-apps/tauri-action` mit
`releaseId` auf. Die Action lädt
dmg, NSIS-Installer, AppImage, deb und rpm an den Entwurf; `includeUpdaterJson` bleibt aus (P6-7). Der Job läuft in
der Umgebung `release`, nutzt aber noch keine Secrets – gebaut wird unsigniert, die Signatur kommt mit P6-6.

**x64-macOS baut nativ** auf `macos-15-intel` statt als Cross-Build: So braucht keine Plattform eine fremde
Rust-Toolchain. ARM-Linux gibt es nicht; die Entscheidung dazu ist offen.

`projectPath` zeigt auf `apps/desktop`, denn dort liegt `@tauri-apps/cli`. Version und Produktname der Dateien
kommen aus dem Crate (`Nexus_<version>_aarch64.dmg`), nicht aus dem Workflow. Dass der Tauri-Build das Binary
über `beforeBuildCommand` (`npm run prepare-binaries`) ein zweites Mal baut, ist gewollt: der eigene Schritt davor
zeigt einen Fehler am Binary früh und baut für genau das gewählte Ziel.

### `cli-archive.mjs`

Packt die CLI für den Betrieb ohne App und gibt den Pfad des Archivs auf stdout aus; der Workflow lädt es mit
`gh release upload` an denselben Entwurf.

```sh
node scripts/release/cli-archive.mjs --version 0.2.0 [--target <rust-ziel>] [--out <ordner>] [--root <dir>]
```

Ergebnis: `dist/release/nexus-cli-<version>-<ziel>.tar.gz` (Windows-Ziel: `.zip`, geschrieben mit `tar -a`). Im
Archiv liegen `bin/nexus[.exe]` – das Rust-Programm, das Server und CLI in einem ist –, das gebündelte Plugin
unter `plugin/` (`build-plugin.mjs` mit `NEXUS_PLUGIN_OUT`) und eine kurze `LIESMICH.md`.
Voraussetzung sind `npm run build` und `cargo build --release -p nexus-cli --bin nexus --target <ziel>`; fehlt
das Binary, bricht das Skript mit diesem Hinweis ab. Die Namensbildung steckt in `lib/CliArchive.mjs` und hat
eigene Tests.

### Lokal prüfen (macOS arm64)

```sh
npm ci && npm run build
cargo build --release -p nexus-cli --bin nexus --target aarch64-apple-darwin
(cd apps/desktop && npx tauri build --target aarch64-apple-darwin --bundles dmg)
ls target/aarch64-apple-darwin/release/bundle/dmg/
node scripts/release/cli-archive.mjs --version 0.0.0-test --target aarch64-apple-darwin
```

Der lokale Build ist unsigniert; Gatekeeper-Warnungen sind dabei erwartet.

## Dateien

| Datei | Aufgabe |
| --- | --- |
| `bump.mjs` | Einstieg: Argumente, Befehle, Ausgabe, Rückgabecode |
| `lib/SemVer.mjs` | Version lesen, schreiben, erhöhen, vergleichen (reine Funktionen) |
| `lib/Tags.mjs` | Versionen aus den Git-Tags `v*`; die Auswahl selbst ist eine reine Funktion |
| `lib/VersionFiles.mjs` | findet alle Versionsstellen, liest sie und setzt sie (Wurzelordner als Parameter) |
| `lib/Changelog.mjs` | Abschnitt umbenennen, leeren Abschnitt einsetzen, Abschnitt einer Version lesen |
| `lib/TomlText.mjs` | TOML zeilenweise lesen und einzelne Schlüssel setzen |
| `lib/JsonText.mjs` | JSON-Werte an ihrer Textstelle setzen, einfügen und entfernen |
| `check-ci.sh` | Branch und CI-Status des Commits prüfen (erster Schritt des Release-Workflows) |
| `version-job.sh` | Version setzen, committen, taggen, pushen |
| `draft-release.sh` | Entwurfs-Release anlegen und seine Id ausgeben |
| `cli-archive.mjs` | CLI und Plugin zu `nexus-cli-<version>-<ziel>.tar.gz` bzw. `.zip` packen |
| `lib/CliArchive.mjs` | Namen des Archivs, des Ordners darin und des Programms (reine Funktionen) |
| `tests/*.test.mjs` | Unit-Tests (`npx vitest run scripts/release`) |
| `tests/RepoCopy.mjs` | kopiert das Fixture-Repo in einen Temp-Ordner |
| `tests/fixtures/repo/` | Mini-Abbild eines Repos mit allen Versionsstellen |

## Tests

```sh
npx vitest run scripts/release
```

Die Tests arbeiten über `--root` auf einer Kopie von `tests/fixtures/repo/` in einem Temp-Ordner, nie im echten
Repo – ein Test darf dessen Version nicht verschieben. Die Tag-Tests legen dort ein eigenes Git-Repo an. Im
echten Repo sind nur die lesenden Befehle gedacht: `check`, `next`, `current` und `notes`.

## Bekannte Grenzen

- **`build-plugin.mjs` bleibt wie es ist.** Der Plugin-Build überschreibt die Version des Manifests mit
  `<major>.<minor>.<zeitstempel>`, damit `claude plugin update` jeden lokalen Build als neueren erkennt. Ein
  Release-Bündel trägt damit nicht die Release-Version – ob es das soll, ist offen (NXS-0021, Abschlusskommentar).
- **Nur `version`-Stellen.** Formatversionen (`lockfileVersion`, `version = 4` in `Cargo.lock`,
  `.claude/launch.json`, `skills-lock.json`) sind keine Release-Versionen und bleiben außen vor.
- **Gemischte Zeilenenden.** Eine Datei mit teils `\n` und teils `\r\n` wird beim Schreiben auf das häufigere
  Zeilenende vereinheitlicht. Dateien mit einheitlichen Zeilenenden behalten sie.
- **Kein Tag und kein Push.** `bump.mjs` setzt nur Versionen; `git commit`, `git tag` und `git push` macht
  `version-job.sh` (siehe „Release-Workflow“).
