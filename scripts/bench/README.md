# Messskript (Ausgangswerte der Migration)

Für die Migration „AI Harness → Nexus“ gilt: keine Messgröße darf schlechter werden als heute. Dieses Skript misst
den Server wiederholbar und legt die Werte in `contract/baseline.json` ab. Es braucht keine zusätzlichen
Abhängigkeiten (kein `hyperfine`) und misst selbst mit `performance.now()`.

Weil der Startbefehl austauschbar ist, misst dasselbe Skript später auch einen anderen Server (Rust) – dann stehen
beide Zahlenreihen im gleichen Format gegenüber.

## Aufruf

```sh
npm run build                                   # der Standardbefehl startet packages/server/dist/main.js
node scripts/bench/bench.mjs --out contract/baseline.json
```

```
node scripts/bench/bench.mjs [--out <datei>] [--server-cmd "<befehl mit {port}>"]
                             [--tickets 500] [--runs 10] [--loads 100] [--quick]
```

| Argument | Bedeutung |
| --- | --- |
| `--out <datei>` | schreibt das Ergebnis zusätzlich in diese Datei; fehlende Ordner werden angelegt |
| `--server-cmd <text>` | Startbefehl des zu messenden Servers; `{port}` wird durch einen freien Port ersetzt. Standard: `node packages/server/dist/main.js --port {port}` |
| `--tickets <zahl>` | Tickets, mit denen das Board befüllt wird (Standard 500) |
| `--runs <zahl>` | Zahl der gemessenen Kaltstarts (Standard 10) |
| `--loads <zahl>` | Zahl der gemessenen warmen Ladevorgänge (Standard 100) |
| `--quick` | kleine Messung: 50 Tickets, 3 Starts, 10 Ladevorgänge – für Tests und als Rauchtest (unter 60 s) |
| `--help` | zeigt die Möglichkeiten |

Ausdrücklich genannte Zahlen gelten auch neben `--quick` (`--quick --tickets 200`). Ohne `--out` steht das Ergebnis
nur auf stdout; der Fortschritt geht immer auf stderr, damit sich stdout als JSON weiterverarbeiten lässt.

Der Aufruf erwartet den Hauptordner des Repos als Arbeitsordner, weil der Standardbefehl einen Pfad darin nennt.

## Ablauf einer Messung

1. Eigenes `HARNESS_HOME` unter dem Temp-Ordner (`harness-bench-…`) und je Start ein freier Port. Der echte
   `~/.harness` und der Standardport 4710 werden nie angefasst; `HARNESS_SESSION` und `HARNESS_TICKET` einer
   laufenden Claude-Session gelten für den gemessenen Server nicht.
2. **Befüllen** (einmalig): Board-Konfig auf SQLite (`<home>/board.db`), Projekt `BEN`, Board `Bench` und dann die
   Tickets über die HTTP-API. Die Tickets verteilen sich auf Backlog, Geplant, In Arbeit, Blockiert und Erledigt,
   jedes fünfte bekommt zwei Kommentare – eine Ansicht mit 500 Karten in einer Spalte misst sonst zu günstig.
3. **Kaltstart** (`--runs` mal): Prozess starten und alle 10 ms fragen, bis `GET /api/health` und danach
   `GET /api/db/boards/<id>/view` mit `200` antworten. Beide Zeiten zählen ab dem Start des Prozesses.
4. **Warme Ladezeit**: Server einmal starten, fünf Aufwärm-Anfragen, dann `--loads` Ladevorgänge der Board-Ansicht
   nacheinander messen (jeweils samt vollständig gelesener Antwort).
5. **Speicher**: 5 s nach dem letzten Ladevorgang den Speicherbedarf (Resident Set Size) des Serverprozesses lesen.
6. Server beenden, Messordner löschen.

## Felder in `contract/baseline.json`

```json
{
  "schema": 1,
  "createdAt": "2026-10-27T10:00:00.000Z",
  "machine": { "os": "darwin", "release": "25.6.0", "arch": "arm64", "cpu": "Apple M…", "cpus": 10, "memGb": 32, "node": "v26.10.0" },
  "server": { "cmd": "node packages/server/dist/main.js --port {port}", "tickets": 500 },
  "metrics": {
    "serverColdStartMs": { "median": 0, "p95": 0, "runs": 10 },
    "boardFirstLoadMs": { "median": 0, "p95": 0, "runs": 10 },
    "boardLoadMs": { "p50": 0, "p95": 0, "mean": 0, "runs": 100 },
    "serverRssIdleMb": { "value": 0 }
  }
}
```

| Feld | Bedeutung |
| --- | --- |
| `schema` | Version des Ausgabeformats |
| `createdAt` | Zeitpunkt der Messung |
| `machine` | Rechner der Messung: Betriebssystem, Ausgabe von `uname -r`, Architektur, Prozessor, Zahl der Kerne, Arbeitsspeicher in GB, Node-Version |
| `server` | gemessener Startbefehl und Zahl der Tickets im Board |
| `serverColdStartMs` | Kaltstart: Millisekunden vom Start des Prozesses bis zur ersten Antwort von `/api/health` (Median, p95, Zahl der Läufe) |
| `boardFirstLoadMs` | „Board sichtbar“ auf Serverseite: Millisekunden vom Start des Prozesses bis zur ersten vollständigen Board-Ansicht (enthält den Kaltstart und das Öffnen der Datenbank) |
| `boardLoadMs` | warme Ladezeit der Board-Ansicht in Millisekunden bei laufendem Server (p50, p95, Mittelwert, Zahl der Ladevorgänge) |
| `serverRssIdleMb` | Speicherbedarf des Serverprozesses im Leerlauf in MB; `null`, wenn der Prozess nicht gefunden wurde |

Alle Werte sind auf eine Nachkommastelle gerundet. Besteht die Datei schon, ersetzt ein Lauf nur die vier Messgrößen
dieses Skripts und seine Angaben zu Rechner und Server; fremde Einträge unter `metrics` (etwa Hook-Laufzeit und
SSE-Latenz aus P0-2) bleiben erhalten.

## Dateien

| Datei | Aufgabe |
| --- | --- |
| `bench.mjs` | Einstieg: Argumente, Ablauf, Ausgabe und das Zusammenführen mit einer vorhandenen Datei |
| `lib/ServerProcess.mjs` | Server starten und beenden, freier Port, auf `200` warten, einzelne Anfrage messen |
| `lib/BoardSeeder.mjs` | Board-Konfig, Projekt, Board, Tickets, Spalten und Kommentare über die HTTP-API |
| `lib/Stats.mjs` | Median, Perzentil, Mittelwert, Runden (reine Funktionen) |
| `lib/ProcessMemory.mjs` | Speicherbedarf eines fremden Prozesses in MB |
| `tests/*.test.mjs` | Unit-Tests der reinen Teile (`npx vitest run scripts/bench`) |

## Bekannte Grenzen

- **Nur der Server.** Der Speicherbedarf des Browser-Tabs (Oberfläche) wird nicht gemessen. Beim Vergleich mit der
  Tauri-Version fehlt diese Hälfte also; sie braucht eine eigene Messung.
- **Nur SQLite.** Gemessen wird gegen eine frische SQLite-Datei, nicht gegen SQL Server.
- **Der Rechner prägt die Zahlen.** Parallele Arbeit (etwa laufende Agenten) macht sie schlechter. Zahlen aus
  verschiedenen Läufen nur vergleichen, wenn `machine` übereinstimmt – am besten auf einem ruhigen Rechner messen.
- **Prozessnummer des Servers.** Der Startbefehl läuft über die Shell, deren Prozessnummer nicht zum Messen des
  Speichers taugt. Das Skript nimmt deshalb die Nummer aus `<home>/server.json` und, falls die Datei fehlt, die aus
  der Antwort von `/api/health`. Ein Server ohne beides liefert `serverRssIdleMb: null`.
- **Keine gleichzeitigen Anfragen.** Die warmen Ladevorgänge laufen nacheinander; Last durch mehrere Tabs ist nicht
  Teil dieser Messung.
