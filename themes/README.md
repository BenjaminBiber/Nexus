# Mitgelieferte Themes

Jeder Unterordner `themes/<id>/` ist die Quelle eines Themes, das Nexus vorinstalliert
mitliefert. Hell und Dunkel sind fest eingebaut und liegen nicht hier.

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
`win95`, `minecraft`) baut das Skript vorher mit `npm run build` und packt dann ihr `dist/`
(`main.js`, `manifest.json`, `theme.css`) statt des Quellordners. `prepare-binaries` der Desktop-App ruft das Skript auf; die
App bündelt `dist/themes/` als Ressource `themes/`. `nexus serve` nimmt in der Entwicklung
`dist/themes/` (oder `--bundled-themes-dir` / `NEXUS_BUNDLED_THEMES_DIR`).

## Theme-Store

Jedes Release hängt die Themes als `<id>-<version>.nexus` und einen Store-Index
`nexus-themes.json` an (Workflow `release.yml`, Skript `scripts/release/theme-index.mjs`).
Im Store als Repo eintragen:

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

## Versionsregel

Beim Start installieren Server und App ein mitgeliefertes Paket nur, wenn das Theme beim Nutzer
fehlt oder in **älterer** Version (semver) installiert ist. Gleiche oder neuere Versionen und
Themes mit anderen IDs bleiben unverändert.

**Wer ein mitgeliefertes Theme ändert, muss `version` im Manifest erhöhen** – sonst bekommen
Nutzer, die es schon haben, die Änderung nicht.
