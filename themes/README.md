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
  claude/**       optional, nur mit "claudeTheme": true – der Mod für Claude Desktop (siehe unten)
```

Versteckte Dateien und Ordner (führender Punkt) und symbolische Links sind nicht erlaubt;
Ordner mit führendem Punkt oder Unterstrich unter `themes/` (z. B. die Vorlage `_template`) werden
übersprungen.

Nexus lädt `main.js` nur, wenn `slots` im Manifest mindestens einen Slot nennt. Ein Theme, das nur
das Mermaid-Aussehen (`ThemeDefinition.mermaid`) mitbringen will, braucht also trotzdem einen Slot;
`dracula` und `alucard` nehmen dafür `Progress` (Fortschrittsring im Ticket-Panel).

## Bauen

```
npm run build:themes
```

packt jeden Ordner mit `nexus theme pack` nach `dist/themes/<id>.nexus` und bricht bei einem
ungültigen Theme mit Exit-Code 1 ab. Code-Themes (`package.json` mit Skript `build`, z. B.
`win95`, `minecraft`, `terminal`, `synthwave`, `dracula`, `alucard`, `gta`) baut das Skript vorher mit `npm run build` und packt dann ihr `dist/`
(`main.js`, `manifest.json`, `theme.css`, `assets/`, `claude/`) statt des Quellordners. Die Pakete sind für Release und
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

## Claude-Theme

Ein Theme kann Claude Desktop mitfärben. Dazu steht im Manifest `"claudeTheme": true`, und unter
`claude/` liegt ein Claude-Code-Mod:

```
claude/hooks/
  hooks.json         { "modules": ["./register.tsx"] }
  register.tsx       ruft registerLook(on, lookFor(settings)) auf
  look.ts            lookFor(settings): Farben (makeColors-Eingabe) und optional eine Szene
  scene.ts           optional: die Szene als SVG-Text
  nexus-look.tsx     ┐
  make-colors.js     ├ Kopien aus themes/_claude-kit/, nicht von Hand bearbeiten
  make-colors.d.ts   ┘
  nexus-settings.ts  Standardwerte aus dem settingsSchema; Nexus überschreibt sie im Slot
```

Unter `claude/` sind zusätzlich `.ts`/`.tsx` erlaubt; ohne `claudeTheme` darf dort nichts liegen,
mit muss `claude/hooks/hooks.json` dabei sein (`nexus theme pack` prüft beides). Die Kit-Dateien
und `nexus-settings.ts` gleicht ein Skript ab; `--check` meldet Abweichungen mit Exit-Code 1:

```
node scripts/themes/sync-claude-kit.mjs [--check]
```

Solange das Theme in Nexus aktiv ist und die Einstellung „Claude übernimmt das Theme“
(`claudeTheme` in den Benutzereinstellungen, Standard an) gilt, legt der Server `claude/` als
Plugin nach `<Claude-Ordner>/skills/nexus-theme/` (`CLAUDE_CONFIG_DIR`, sonst `~/.claude`), mit
`.claude-plugin/plugin.json`, der Marke `.nexus-theme.json` und den gewählten Theme-Einstellungen in
`hooks/nexus-settings.ts`. Claude Desktop beobachtet den Ordner und lädt neu. Theme-Wechsel,
Schalter, Theme-Einstellungen, Installation, Update und Entfernen gleichen den Slot ab; einen
Ordner ohne Marke fasst Nexus nicht an und meldet ihn unter Darstellung › Themes › Claude Desktop.

Farben: `makeColors` darf keine Zeile „Desktop won't paint“ melden (Test je Theme). Szenen sind
eigene Kunst, liegen am Rand und nicht hinter Text oder Eingabefeld, haben `viewBox` mit
`preserveAspectRatio="xMidYMid slice"` und bleiben unter 131.072 Zeichen. An den Rand heften lässt
sich ein Motiv unter `slice` nur über `@media (min-aspect-ratio …)` / `(max-aspect-ratio …)` im
eigenen `<style>`, nicht über verschachtelte `<svg>`. Prüfen lässt sich ein gebauter Slot mit
`claude plugin validate <ordner>` (Claude Code ab 2.1.325).

## Versionsregel

Der Store bietet ein Update an, wenn das Release eine **neuere** Version (semver) hat als die
installierte. **Wer ein Theme ändert, muss `version` im Manifest erhöhen** – sonst bekommen
Nutzer, die es schon haben, die Änderung nicht.
