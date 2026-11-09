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
Ordner mit führendem Punkt unter `themes/` werden übersprungen.

## Bauen

```
npm run build:themes
```

packt jeden Ordner mit `nexus theme pack` nach `dist/themes/<id>.nexus` und bricht bei einem
ungültigen Theme mit Exit-Code 1 ab. `prepare-binaries` der Desktop-App ruft das Skript auf; die
App bündelt `dist/themes/` als Ressource `themes/`. `nexus serve` nimmt in der Entwicklung
`dist/themes/` (oder `--bundled-themes-dir` / `NEXUS_BUNDLED_THEMES_DIR`).

## Versionsregel

Beim Start installieren Server und App ein mitgeliefertes Paket nur, wenn das Theme beim Nutzer
fehlt oder in **älterer** Version (semver) installiert ist. Gleiche oder neuere Versionen und
Themes mit anderen IDs bleiben unverändert.

**Wer ein mitgeliefertes Theme ändert, muss `version` im Manifest erhöhen** – sonst bekommen
Nutzer, die es schon haben, die Änderung nicht.
