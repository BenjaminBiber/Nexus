# Paketierung der Desktop-App

Nexus wird mit Tauri 2 paketiert. Die Konfiguration steht in `apps/desktop/src-tauri/tauri.conf.json`
unter `bundle`; gebaut wird je Plattform auf einem Rechner dieser Plattform (Cross-Builds nur zwischen den
beiden macOS-Architekturen). Signatur und Updater sind nicht Teil dieser Konfiguration.

## Was in ein Paket gehört

Jedes Paket enthält

- das Hauptprogramm (`nexus-desktop`),
- das Sidecar `harness-sidecar` (`bundle.externalBin`), gebaut mit `npm run build:sidecar`,
- die drei Ressourcenordner `web/`, `toast/` und `sql/` (`bundle.resources`, Quelle `dist/sidecar/`).

Alles davon stellt `npm run prepare-sidecar -w apps/desktop` her; Tauri ruft das über
`build.beforeBuildCommand` selbst auf. Das Sidecar ist eine Node Single Executable Application und braucht
beim Bauen ein eigenständiges Node von nodejs.org – ein Node aus einer Paketverwaltung (etwa Homebrew) ist
als Shared Library gebaut und kann das nicht. Prüfen mit
`node -p "process.config.variables.node_shared"` (muss `false` sein).

## macOS

```bash
npm run tauri -w apps/desktop -- build --bundles app,dmg
node apps/desktop/scripts/check-bundle.mjs
hdiutil verify target/release/bundle/dmg/Nexus_*_aarch64.dmg
```

Ergebnis unter `target/release/bundle/`: `macos/Nexus.app` und `dmg/Nexus_<version>_aarch64.dmg`.
`check-bundle.mjs` prüft Hauptprogramm, Sidecar, die drei Ressourcenordner und das Symbol im `.app` und gibt
die Größe des Installers aus; bei einem Fehlbefund endet es mit Code 1.

Für Intel-Macs (das Release baut sie nicht, lokal geht es weiterhin) zusätzlich (einmalig `rustup target add x86_64-apple-darwin`, dazu ein Node von nodejs.org
für dasselbe Ziel):

```bash
node scripts/sidecar/build-sidecar.mjs --target x86_64-apple-darwin --node-binary <pfad/zu/node-x64>
npm run tauri -w apps/desktop -- build --target x86_64-apple-darwin --bundles dmg
node apps/desktop/scripts/check-bundle.mjs x86_64-apple-darwin
```

Das `.dmg` ist unsigniert. Beim ersten Start warnt Gatekeeper; die App wird über „Öffnen“ im Kontextmenü
bzw. in den Systemeinstellungen freigegeben. Signatur und Notarisierung kommen später.

## Windows

```powershell
npm run tauri -w apps/desktop -- build --bundles nsis
```

Ergebnis: `target\release\bundle\nsis\Nexus_<version>_x64-setup.exe`.

- `nsis.installMode: "currentUser"` – Installation ins Benutzerprofil, keine Administratorrechte nötig.
- `webviewInstallMode: { "type": "downloadBootstrapper" }` – fehlt WebView2, lädt der Installer den
  Bootstrapper von Microsoft nach. Der Rechner braucht dafür einen Internetzugang.
- Sprachen: Deutsch und Englisch, ohne Sprachauswahl beim Start (die Sprache des Systems gewinnt).
- `nsis.installerHooks` (`src-tauri/windows/installer-hooks.nsh`) – trägt den Installationsordner
  (`%LOCALAPPDATA%\Nexus`, dort liegt `nexus.exe`) in den Benutzer-PATH ein und beim Deinstallieren
  wieder aus. Bereits offene Terminals sehen den Eintrag erst nach einem Neustart.
- Aussehen: `installerIcon` ist das App-Icon, `sidebarImage` (164×314, Willkommens- und Abschlussseite)
  und `headerImage` (150×57, Kopf der übrigen Seiten) zeigen den Schriftzug `assets/brand/nexus-logo.svg`
  im Markenblau `#1d4ed8`. Die BMPs in `src-tauri/windows/` erzeugt `assets/brand/render.mjs` zusammen mit
  den übrigen Markenbildern (Aufruf siehe Kopf der Datei).

## Linux

```bash
npm run tauri -w apps/desktop -- build --bundles appimage,deb,rpm
```

Ergebnis unter `target/release/bundle/`: `appimage/`, `deb/` und `rpm/`.

Zum Bauen nötig (Ubuntu 22.04 als Baubasis, damit die glibc alt genug für ältere Systeme ist):

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf \
  build-essential curl wget file libssl-dev libgtk-3-dev
```

`rpm` braucht zusätzlich `rpm` (Debian/Ubuntu: `sudo apt-get install -y rpm`).

Zur Laufzeit verlangen die Pakete WebKitGTK 4.1 und den AppIndicator für das Symbol im Infobereich; das
steht als `depends` in der Konfiguration (`deb`: `libwebkit2gtk-4.1-0`, `libayatana-appindicator3-1`;
`rpm`: `webkit2gtk4.1`, `libayatana-appindicator-gtk3`). Das AppImage bringt keine Medienbibliotheken mit
(`bundleMediaFramework: false`).

## Dateinamen

Diese Namen erwartet der Release-Workflow:

| Plattform | Datei |
|---|---|
| macOS (Apple Silicon) | `Nexus_<version>_aarch64.dmg` |
| Windows | `Nexus_<version>_x64-setup.exe` |
| Linux AppImage | `Nexus_<version>_amd64.AppImage` |
| Linux deb | `Nexus_<version>_amd64.deb` |
| Linux rpm | `Nexus-<version>-1.x86_64.rpm` |

Nachgemessen ist bisher nur der macOS-Name für Apple Silicon; die übrigen stammen aus den Regeln des
Bundlers und werden mit dem ersten CI-Build bestätigt.

## Symbole

Quelle ist `apps/desktop/app-icon.svg` (1024×1024). Alle Größen entstehen daraus:

```bash
npm run tauri -w apps/desktop -- icon app-icon.svg
```

Das überschreibt die Dateien in `apps/desktop/src-tauri/icons/` und legt zusätzlich Ordner für iOS und
Android an; die werden nicht gebraucht und wieder gelöscht. Die Symbole für den Infobereich unter
`icons/tray/` erzeugt `tauri icon` nicht und fasst sie auch nicht an.

## Größen

Stand 0.1.0 auf Apple Silicon, ohne das echte Sidecar gemessen (Platzhalter statt der Node-Datei):
`Nexus.app` 25,5 MiB, `Nexus_0.1.0_aarch64.dmg` 9,9 MiB. Das echte Sidecar enthält ein vollständiges
Node-Programm und wächst beide Werte um gut 100 MB. Verbindliche Größen liefert der erste vollständige
Build im Release-Workflow.
