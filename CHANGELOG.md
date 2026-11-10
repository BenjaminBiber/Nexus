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
- „Theme installieren“ ist ein Kombi-Knopf: Das Dreipunkt-Menü daneben öffnet „Theme-Stores
  verwalten“ (statt des Plus-Knopfs neben der Suche).
- Der Dialog „Theme installieren“ ist neu gestaltet: aufklappbarer Zustand der Theme-Stores
  (erreichbar oder nicht, mit Fehlercode, Erklärung, „Releases öffnen“ bei GitHub und „Erneut
  prüfen“), Suche mit Anzahl und Kacheln mit Vorschaubild, Version, Store und Installieren.
- Die Rückfrage vor dem Installieren oder Aktualisieren ist ein eigener Dialog über dem Store
  statt eines Kastens darin; Esc schließt nur die Rückfrage.
- Der Zustand der Theme-Stores im Dialog „Theme installieren“ ist anfangs eingeklappt; Fehler
  zeigt die Kopfzeile auch eingeklappt an.
- Der Fokusrahmen des Kombi-Knopfs „Theme installieren“ umschließt beide Hälften statt nur der
  fokussierten.
- „Windows 95“ 1.3.0: Themes-Seite und Theme-Store im Win95-Stil – Theme-Kacheln als Knöpfe
  (aktuelles eingedrückt), Plaketten als eingelassene Felder, Kombi-Knopf mit Menü, Dialoge mit
  blauer Titelleiste und Kreuzknopf, Store-Zustand als Gruppenrahmen mit Listenansicht,
  Store-Kacheln mit „Installieren“-Knopf und Rückfragen als Meldungsfenster.
- „Terminal“ 2.0.0 (braucht SDK 1.2): Einstellungen unter Darstellung für Schrift (JetBrains Mono,
  IBM Plex Mono oder Geist Mono, 12–14 px, Ligaturen), Symbole (Nerd Font, Unicode oder keine),
  acht Farbschemata (GitHub Dark, Dracula, Gruvbox, Tokyo Night, Solarized, Nord, Phosphor grün
  und bernstein) und Effekte (Röhrenmonitor, Cursor-Blinken, getippte Ausgabe, Boot-Meldungen).
  Prompt- und Statuszeile im Powerline-Stil mit Status des letzten Befehls, Docs-Baum mit Linien
  wie `tree`, Fortschritt als `[#####·····]`, eckige Mermaid-Diagramme und neue Befehle `man`,
  `top`, `fetch`/`neofetch`, `history` und `cowsay`. Schriften liegen im Paket (OFL 1.1, Nerd-Font-
  Teilmenge MIT).
- „Terminal“ 2.1.0: Themes-Seite und Theme-Store im Terminal-Stil – eckige Kacheln mit Linie
  (aktuelles mit `> ` und grüner Fläche), Plaketten als `[Text]`, Filter wie Segmente, Kombi-Knopf
  als `[ Theme installieren | ⋯ ]`, Dialoge mit `## `-Überschrift und `[x]`, Store-Zustand mit
  eckigen Punkten und Store-Kacheln mit `[ Installieren ]`.
- Neues Theme „Synthwave“ 1.0.0 im Theme-Store (`themes/synthwave/`, Code-Theme): Neon der 80er
  auf dunklem Violett mit eigener Shell – Rahmen mit Neon-Gitter am Horizont, Sonne und Himmel
  nach Tageszeit, Navigation mit nummerierten Einträgen, Fortschritt als Laserbalken und
  Mermaid-Diagramme im Neon-Look. Beim Laden geht die Sonne über dem Gitter auf (abschaltbar).
  Einstellungen: Farbschema (Outrun, Miami, Nightcall), Stärke des Leuchtens, Horizont, Himmel
  und VHS-Effekte (Scanlines, Bandzähler, Bildstörung). Schriften im Paket (OFL 1.1).
- „Synthwave“ 1.1.0: hellere Flächen, Linien und Display oben für mehr Kontrast; das Neon-Gitter
  steht auf dunklem Boden mit leuchtender Horizontlinie und reicht bis an den Horizont; nachts
  hängt ein Mond am Himmel. Fokus und Überfahren einheitlich (Felder mit einem Magenta-Rahmen statt
  Doppelrahmen, Suchfelder ohne inneren Rand, Navigation der Einstellungen wie die Titelliste). In
  den Einstellungen bleibt die Navigation stehen, nur die Seite scrollt. Aktive Einträge in
  Titelliste, Einstellungen und Menüs ohne Kante links: der laufende Titel sieht aus wie eine
  eingelegte Kassette (Rahmen im Sonnenverlauf, Etikettstreifen unten). Die Suche oben nimmt die
  volle Breite ein und ist ein Leuchtdisplay mit blinkendem Cursor; Display und Deck-Leiste sind
  heller und mit Neonlinien abgesetzt, der Plus-Knopf leuchtet in Magenta, die VHS-Vignette
  dunkelt die Ränder nur noch leicht ab.
  Durchgängig wie ein Gerät der 80er: alle Seitenüberschriften in Chrom-Orbitron, Filter und
  Umschalter als Tapedeck-Tasten mit LED, Kontrollkästchen als LED-Schalter, Kennzahlen im
  Dashboard als Leuchtziffern, Diagramme auf Neon-Millimeterpapier (Mermaid: Knoten als
  Leuchtschilder, auf den Kanten läuft ein Signal, Beschriftungen und Notizen als Bandetiketten),
  Code-Zeilen als Terminal, Tasten als Gerätetasten, Ticket-Panel mit den Streifen einer
  Kassettenhülle, Suche mit „◀◀ SEARCH ▶▶“, Scrollbalken im Neonverlauf. Versteckte Details: die
  Kassette lässt sich über „SIDE A“ umdrehen (Seite B ist die Nachtfahrt, Titel B1, B2 …), mit
  VHS dreht sie sich nach 45 Minuten von selbst (Auto-Reverse); im Schriftzug setzt ab und zu
  eine Neonröhre aus; nachts fallen Sternschnuppen; bei getrennter Verbindung steht „OFF AIR“;
  ein voller Laserbalken meldet „STAGE CLEAR“; und wer den Konami-Code kennt, schaltet in den
  Overdrive. Bei reduzierter Bewegung bleibt alles ruhig.
- Neue Themes „Dracula“ 1.0.0 (`themes/dracula/`, dunkel) und „Alucard“ 1.0.0
  (`themes/alucard/`, hell) im Theme-Store: die Standard-Oberfläche in der Dracula-Palette bzw.
  ihrem hellen Gegenstück Alucard (draculatheme.com, MIT), Alucard bewusst farbig, aber ruhig:
  lavendelgetönte Flächen, Knöpfe in Purple, Seitentitel im Verlauf von Purple zu Pink, im Board
  Spalten ohne Fläche, flach in der Statusfarbe getönte Spaltenköpfe und ein Rahmen in dieser
  Farbe um jede Karte, dazu in der Navigation ein schlichter
  Unterstrich für den aktiven Eintrag. Mermaid-Diagramme in den Farben der Palette (Knoten
  mit Purple-Rand, Kanten in Cyan, Notizen in Yellow, Kreisdiagramme und Git-Graphen in den
  Akzentfarben), der Fortschrittsring im Ticket-Panel mit Verlauf von Purple nach Pink; Rahmen,
  Symbole und alles andere bleiben Standard. Schrift und Statusfarben erreichen mindestens 4,5:1
  Kontrast.

## 0.0.3 – 2026-11-09

### Vertrag

- Keine Änderungen.

### Angekündigt

- Keine.

### Sonstiges

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
