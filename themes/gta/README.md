# GTA-Theme

Code-Theme, das Nexus wie einen Teil des Spiels aussehen lässt. Es ersetzt die Slots `Shell`, `NavIcon`, `Icon`,
`Progress` und `StartAnimation`; die echten Seiten von Nexus bleiben erhalten und werden per CSS im selben Look
gestaltet. Zwei Stile, umschaltbar unter **Einstellungen → Darstellung → Einstellungen des Themes**:

- **GTA V** (Standard): Pausenmenü mit Reiterzeile, HUD oben rechts (Board, Uhr, Sterne, Geld), eckige Minimap unten
  links mit drei Balken.
- **San Andreas**: Menüliste links, gotischer Seitentitel, HUD-Block oben rechts (Faust, Uhr, Panzerung, Gesundheit,
  Geld, Sterne), rundes Radar unten links.

## Einstellungen

| Einstellung | Werte | Wirkung |
|---|---|---|
| Stil | GTA V, San Andreas | Rahmen im Look des Pausenmenüs von GTA V oder der Menüliste von San Andreas |
| Akzentfarbe | Blau, Grün, Orange | Farbe der Auswahl in Menü und Reitern |
| Radar | an/aus | Minimap bzw. Radar unten links mit den Tickets des Boards |
| Banner | an/aus | Mission Passed, Busted, Wasted und Handy-Meldungen zu neuen Fragen |
| Ladebildschirm | an/aus | Ladebildschirm beim Öffnen der Oberfläche |
| Cheats | an/aus | Tastenfolgen wie HESOYAM; ändern nur die Anzeige |

## Was die Anzeigen bedeuten

- **Geld** ist Spielgeld, keine echten Kosten: 40.000 je erledigtem Ticket, 2.500 je erledigtem Schritt offener
  Tickets. Nicht benötigte Tickets zählen nicht.
- **Sterne:** Zahl der blockierten Tickets (höchstens fünf in GTA V bzw. sechs in San Andreas).
- **Balken:** in GTA V unter der Minimap der Fortschritt des Boards (Anteil erledigter Tickets) und der Rest des
  5-Stunden- und des 7-Tage-Kontingents von Claude; in San Andreas stehen Panzerung und Gesundheit für die beiden
  Kontingente.
- **Radar bzw. Minimap:** ein Punkt je offenem Ticket des Boards; blockierte Tickets fallen auf.
- **Banner:** „Mission Passed“, wenn ein Ticket erledigt ist, „Busted“, wenn eines blockiert wird (mit Grund), „Wasted“,
  wenn das 5-Stunden-Kontingent aufgebraucht ist.
- **Handy:** meldet neue Fragen; Annehmen öffnet die Seite „Fragen“.
- **Stats-Menü:** Kennzahlen des Boards im Look der Statistikseite des Spiels.

## Tasten

- **Q** / **E** (GTA V): vorheriger bzw. nächster Reiter.
- **Y** / **N** (San Andreas, nur während eines Anrufs): Anruf annehmen (zur Seite „Fragen“) bzw. ablehnen.
- **Cheats** (Buchstaben nacheinander tippen, außerhalb von Feldern):
  - `HESOYAM`, `TURTLE`: 250.000 Spielgeld mehr bis zum Neuladen.
  - `AEZAKMI`, `LAWYERUP`: Sterne auf null.
  - `BRINGITON`, `FUGITIVE`: alle Sterne (fünf in GTA V, sechs in San Andreas).

  Die Sterne gelten, bis sich die Zahl blockierter Tickets ändert. Cheats ändern nur die Anzeige, nie das Board.

Tasten werden nie in Feldern oder mit Strg/Alt/Meta ausgewertet; sie ruhen, solange ein Dialog, ein Seitenblatt, ein
Menü oder die Suche offen ist oder der Ladebildschirm läuft. Tab und Pfeiltasten bleiben frei. Es gibt keine
Klänge. Bei „Bewegung reduzieren“ (`prefers-reduced-motion`) stehen Animationen still.

## Herkunft

Die Schriften liegen unter `assets/fonts` (Teilmenge latin, Quelle Google Fonts):

| Stil | Familie | Lizenz |
|---|---|---|
| GTA V | Barlow, Barlow Condensed | SIL Open Font License 1.1 |
| GTA V | Lilita One | SIL Open Font License 1.1 |
| GTA V | Yellowtail | Apache License 2.0 |
| San Andreas | Arimo | SIL Open Font License 1.1 |
| San Andreas | Michroma | SIL Open Font License 1.1 |
| San Andreas | Passion One | SIL Open Font License 1.1 |
| San Andreas | Pirata One | SIL Open Font License 1.1 |

Urheber stehen im Kopfkommentar von `theme.css`. Copyright und Lizenz je Schrift sowie der vollständige Text beider
Lizenzen liegen im Paket unter `assets/fonts/licenses.json`. Die Wortmarken sind aus diesen freien Schriften erzeugte Pfade
(`src/shared/wordmarks.ts`); Szenen, Symbole, Radar, Faust und Handy sind selbst gezeichnetes SVG/CSS. Das Paket enthält
**kein Originalmaterial** des Spiels und lädt nichts nach (`permissions.network` ist leer).

„Grand Theft Auto“ ist eine Marke von Take-Two Interactive. Dieses Theme ist ein Fan-Projekt ohne Verbindung zu
Rockstar Games.

## Entwickeln

```
cd themes/gta
npm run build        # Vite-Build nach dist/
npm run typecheck
npx vitest run themes/gta   # aus dem Wurzelordner des Repos
nexus theme pack dist
```

Alle Themes auf einmal packen (aus dem Wurzelordner): `npm run build:themes`.
