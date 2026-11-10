# Changelog

Änderungen am Harness. Der Abschnitt „Vertrag“ nennt, was Anpassungen in `custom/` nutzen dürfen und was stabil bleibt. Änderungen daran stehen zuerst unter „Angekündigt“ und gelten frühestens eine Version später.

## Unveröffentlicht

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

- Jedes Release enthält die Themes aus `themes/` als `<id>-<version>.nexus` und den Store-Index
  `nexus-themes.json`. Der Theme-Store hat dieses Repo
  (`https://github.com/BenjaminBiber/Nexus/releases/latest/download/nexus-themes.json`)
  vorbelegt, solange der Nutzer seine Repo-Liste nie geändert hat; entfernt er es, bleibt es weg.
  Die SDK-Version, gegen die der Store `sdkVersion` prüft, ist jetzt 1.1.0 (wie im Theme-SDK).
- Mitgeliefert werden nur Hell, Dunkel und System. Alle Themes aus `themes/` zeigt der Store an,
  installiert werden sie erst per Download. `npm run build:themes` packt `themes/<id>/` nach
  `dist/themes/<id>.nexus` (für Release und Store-Index); die Desktop-App bündelt sie nicht.
  `nexus serve` kann Pakete aus einem Ordner beim Start installieren, nur wenn er ausdrücklich
  angegeben ist (`--bundled-themes-dir`, `NEXUS_BUNDLED_THEMES_DIR`). Ordner mit führendem
  Unterstrich (Vorlage `_template`) werden nicht gepackt.
- Das Theme „Signal“ ist kein eingebautes Theme mehr, sondern ein Paket im Theme-Store
  (`themes/signal/`); eine gespeicherte Auswahl `signal` bleibt gültig, solange das Paket
  installiert ist. Die Fehlermeldung zu `theme` nennt `signal` nicht mehr unter den eingebauten.
- Das Theme „Pastell“ ist ebenfalls ein Paket im Theme-Store (`themes/pastel/`) statt im Kern;
  eine gespeicherte Auswahl `pastel` bleibt gültig, solange das Paket installiert ist.
- „Windows 95“ ist ein Theme-Paket mit Code im Theme-Store (`themes/win95/`: `main.js` aus
  `src/` per Vite gebaut, dazu `theme.css`); im Kern stecken nur noch Hell und Dunkel. Eine
  gespeicherte Auswahl `win95` bleibt gültig, solange das Paket installiert ist. Die Fehlermeldung
  zu `theme` nennt `win95` nicht mehr unter den eingebauten.
- `npm run build:themes` baut Themes mit `package.json` und Skript `build` zuerst (Vite) und
  packt dann deren `dist/` statt des Quellordners.
- Neue Themes aus AI Harness unter `themes/`: „Catppuccin“ (`themes/catppuccin/`, dunkle
  Mocha-Palette, reines CSS) und „Terminal“ (`themes/terminal/`, Code-Theme mit eigener Shell:
  Prompt-Zeile mit Befehlen wie `help`, `cd`, `ls`, `open`, `board`, `theme`, Navigation als
  Liste, Statuszeile, Festbreitenschrift).
- „Windows 95“ 1.2.0: vollständiger Stand aus AI Harness – Suchfelder, Leerzustand, „Meine
  Aufgaben“, Leiste an der Textauswahl, Docs im Explorer-/WordPad-Stil, Pixel-Symbole auch im
  Docs-Baum (Dokument, Notizblock, Buch, Urkunde, Ordner), im Leerzustand (Hinweis) und an der
  Textauswahl (Bleistift, Sprechblase) sowie Mermaid-Diagramme wie erhabene Knöpfe. Braucht SDK 1.2.
- Theme-SDK 1.2.0: neuer Slot `Icon` (`IconName`: Symbole im Docs-Baum, im Leerzustand und an der
  Textauswahl in Docs; Ordner im Docs-Baum zeigen ohne Slot weiterhin kein Symbol) und neues Feld
  `mermaid` der `ThemeDefinition` für das Aussehen von Mermaid-Diagrammen. Der Store prüft
  `sdkVersion` jetzt gegen 1.2.0.
- „Pastell“ wird zu „Paper Notes“ 2.0.0 (Punktpapier, Handschrift, gezeichnete Konturen,
  Textmarker); die ID bleibt `pastel`, eine gespeicherte Auswahl gilt weiter.
- Die Themes bringen ein Vorschaubild mit (`assets/preview.png`, Board mit Demodaten), das die
  Kachel unter Einstellungen → Darstellung und der Theme-Store zeigen. Der Store-Index
  veröffentlicht Vorschaubilder aus dem Paket am Release als `<id>-<version>-preview.png` und
  verlinkt sie unter `preview`. Minecraft bleibt ohne Bild (Mojang-Texturen werden nicht
  mitverteilt). Aufnahme mit `scripts/screenshots/theme-previews.mjs`; die Demodaten der
  README-Screenshots liegen jetzt gemeinsam in `scripts/screenshots/demo.mjs`.
- Die Fensterleiste von „Windows 95“ heißt „Nexus“ statt „AI Harness“.

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
