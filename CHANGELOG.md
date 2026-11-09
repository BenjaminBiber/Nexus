# Changelog

Änderungen am Harness. Der Abschnitt „Vertrag“ nennt, was Anpassungen in `custom/` nutzen dürfen und was stabil bleibt. Änderungen daran stehen zuerst unter „Angekündigt“ und gelten frühestens eine Version später.

## Unveröffentlicht

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

- Entwicklungsserver (`npm run dev:web`): Die Import-Map für Code-Themes gibt es jetzt auch dort –
  `react`, `react-dom`, `react-dom/client`, `react/jsx-runtime` und `@nexus/theme-sdk` zeigen auf
  virtuelle Module von Vite (dieselbe React-Kopie wie die App). Zusätzlich leitet Vite `/themes` an
  den Server auf Port 4710 weiter, damit Manifest und `main.js` installierter Themes ankommen.
  Code-Themes lassen sich damit ohne vorherigen Build ausprobieren.
- Jedes Release enthält die mitgelieferten Themes als `<id>-<version>.nexus` und den Store-Index
  `nexus-themes.json`; im Theme-Store als Repo eintragbar unter
  `https://github.com/BenjaminBiber/Nexus/releases/latest/download/nexus-themes.json`. Die
  SDK-Version, gegen die der Store `sdkVersion` prüft, ist jetzt 1.1.0 (wie im Theme-SDK).
- Mitgelieferte Themes: `npm run build:themes` packt `themes/<id>/` nach `dist/themes/<id>.nexus`;
  die Desktop-App bündelt sie, und `nexus serve` sowie die App installieren beim Start jedes
  Paket, dessen Theme fehlt oder älter ist (`--bundled-themes-dir`, `NEXUS_BUNDLED_THEMES_DIR`).
  Ordner mit führendem Unterstrich (Vorlage `_template`) werden nicht gepackt.
- Das Theme „Signal“ ist kein eingebautes Theme mehr, sondern ein mitgeliefertes Paket
  (`themes/signal/`); eine gespeicherte Auswahl `signal` bleibt gültig, solange das Paket
  installiert ist. Die Fehlermeldung zu `theme` nennt `signal` nicht mehr unter den eingebauten.
- Das Theme „Pastell“ ist ebenfalls ein mitgeliefertes Paket (`themes/pastel/`) statt im Kern;
  eine gespeicherte Auswahl `pastel` bleibt gültig, solange das Paket installiert ist.
- „Windows 95“ ist ein mitgeliefertes Theme-Paket mit Code (`themes/win95/`: `main.js` aus
  `src/` per Vite gebaut, dazu `theme.css`); im Kern stecken nur noch Hell und Dunkel. Eine
  gespeicherte Auswahl `win95` bleibt gültig, solange das Paket installiert ist. Die Fehlermeldung
  zu `theme` nennt `win95` nicht mehr unter den eingebauten.
- `npm run build:themes` baut Themes mit `package.json` und Skript `build` zuerst (Vite) und
  packt dann deren `dist/` statt des Quellordners.

## 0.0.2 – 2026-11-06

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

## 0.0.1 – 2026-11-05

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

- Erste öffentliche Version.
