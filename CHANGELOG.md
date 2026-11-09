# Changelog

Änderungen am Harness. Der Abschnitt „Vertrag“ nennt, was Anpassungen in `custom/` nutzen dürfen und was stabil bleibt. Änderungen daran stehen zuerst unter „Angekündigt“ und gelten frühestens eine Version später.

## Unveröffentlicht

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

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
