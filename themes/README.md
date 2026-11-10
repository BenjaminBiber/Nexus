# Themes im Theme-Store

Jeder Unterordner `themes/<id>/` ist die Quelle eines Themes, das Nexus über seinen Theme-Store
anbietet. Mitgeliefert werden nur Hell, Dunkel und System; sie sind fest eingebaut und liegen
nicht hier. Alle Themes von hier zeigt der Store an, installiert werden sie erst per Download.

## Aufbau

```
themes/<id>/
  manifest.json   Pflicht; "id" muss dem Ordnernamen entsprechen
  theme.css       Pflicht; kein @import, url() nur relativ oder data:
  main.js         optional, ESM
  assets/**       optional (Bilder, Schriften, Klänge)
```

Versteckte Dateien und Ordner (führender Punkt) und symbolische Links sind nicht erlaubt;
Ordner mit führendem Punkt oder Unterstrich unter `themes/` (z. B. die Vorlage `_template`) werden
übersprungen.

## Bauen

```
npm run build:themes
```

packt jeden Ordner mit `nexus theme pack` nach `dist/themes/<id>.nexus` und bricht bei einem
ungültigen Theme mit Exit-Code 1 ab. Code-Themes (`package.json` mit Skript `build`, z. B.
`win95`, `minecraft`, `terminal`, `synthwave`) baut das Skript vorher mit `npm run build` und packt dann ihr `dist/`
(`main.js`, `manifest.json`, `theme.css`, `assets/`) statt des Quellordners. Die Pakete sind für Release und
Store-Index; die Desktop-App bündelt sie nicht. Wer sie lokal vorinstallieren will, gibt den Ordner
`nexus serve` ausdrücklich mit (`--bundled-themes-dir dist/themes` / `NEXUS_BUNDLED_THEMES_DIR`).

## Theme-Store

Jedes Release hängt die Themes als `<id>-<version>.nexus` und einen Store-Index
`nexus-themes.json` an (Workflow `release.yml`, Skript `scripts/release/theme-index.mjs`).
Dieses Repo ist im Store vorbelegt (`NEXUS_STORE_REPO` in
`crates/nexus-core/src/theme/store_repos.rs`), solange der Nutzer seine Repo-Liste nie geändert
hat; entfernt er es, bleibt es weg, und er kann es von Hand wieder eintragen:

```
https://github.com/BenjaminBiber/Nexus/releases/latest/download/nexus-themes.json
```

`downloadUrl` jedes Eintrags zeigt auf das Tag des Releases, `sha256` auf die Bytes genau
dieses Pakets. Die Pakete sind nicht signiert (im Store `verified: false`). Ablauf für eine
neue Theme-Version: `version` im Manifest erhöhen, dann Release. Lokal probieren:

```
npm run build:themes
node scripts/release/theme-index.mjs dist/themes http://127.0.0.1:8080 dist/release-themes
npx serve -l 8080 dist/release-themes   # dann http://127.0.0.1:8080/nexus-themes.json eintragen
```

## Vorschaubild

Jedes Theme bringt `assets/preview.png` mit (im Manifest `"preview": "assets/preview.png"`): das
Board „Nova Shop“ aus erfundenen Demodaten, 640×320. Die Einstellungen zeigen es auf der Kachel
installierter Themes; `theme-index.mjs` legt es am Release als `<id>-<version>-preview.png` neben
das Paket und trägt die URL als `preview` in den Store-Index ein. Neu aufnehmen (Voraussetzungen
wie bei den Screenshots im Haupt-README, dazu `npm run build:themes`):

```
node scripts/screenshots/theme-previews.mjs [<id> …]
```

Themes mit fremden Ressourcen (`permissions.network`, z. B. Minecraft mit Mojang-Texturen)
bekommen kein Bild: mit den Texturen dürfte es weder ins Repo noch ins Paket, ohne sie zeigt es
nur die Rückfall-Ansicht. Ein neues Bild ist eine Änderung am Paket, also `version` erhöhen.

## Versionsregel

Der Store bietet ein Update an, wenn das Release eine **neuere** Version (semver) hat als die
installierte. **Wer ein Theme ändert, muss `version` im Manifest erhöhen** – sonst bekommen
Nutzer, die es schon haben, die Änderung nicht.
