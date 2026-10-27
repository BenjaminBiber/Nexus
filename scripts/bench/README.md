# Messskript (Ausgangswerte der Migration)

Für die Migration „AI Harness → Nexus“ gilt: keine Messgröße darf schlechter werden als heute. Dieses Skript misst
Server, Hooks und den Ereignisstrom wiederholbar und legt die Werte in `contract/baseline.json` ab. Es braucht keine
zusätzlichen Abhängigkeiten (kein `hyperfine`) und misst selbst mit `performance.now()`.

Weil der Startbefehl austauschbar ist, misst dasselbe Skript später auch einen anderen Server (Rust) – dann stehen
beide Zahlenreihen im gleichen Format gegenüber. Für die Hooks gilt dasselbe über `--hook-cmd`.

## Aufruf

```sh
npm run build                                   # der Standardbefehl startet packages/server/dist/main.js
node scripts/bench/bench.mjs --out contract/baseline.json
node scripts/bench/bench.mjs --quick --compare contract/baseline.json   # Code 1, wenn etwas schlechter ist
```

```
node scripts/bench/bench.mjs [--out <datei>] [--server-cmd "<befehl mit {port}>"] [--hook-cmd "<befehl>"]
                             [--tickets 500] [--runs 10] [--loads 100]
                             [--hook-runs 30] [--sse-runs 20] [--db-runs 10]
                             [--quick] [--hyperfine] [--compare <datei> [--tolerance 10]]
```

| Argument | Bedeutung |
| --- | --- |
| `--out <datei>` | schreibt das Ergebnis zusätzlich in diese Datei; fehlende Ordner werden angelegt |
| `--server-cmd <text>` | Startbefehl des zu messenden Servers; `{port}` wird durch einen freien Port ersetzt. Standard: `node packages/server/dist/main.js --port {port}` |
| `--hook-cmd <text>` | Befehl des Hooks ohne das Ereignis (es wird angehängt). Standard: das Plugin-Bündel dieses Repos, in einen temporären Ordner gebaut. Später z. B. `--hook-cmd "nexus hook"` |
| `--tickets <zahl>` | Tickets, mit denen das Board befüllt wird (Standard 500) |
| `--runs <zahl>` | Zahl der gemessenen Kaltstarts (Standard 10) |
| `--loads <zahl>` | Zahl der gemessenen warmen Ladevorgänge (Standard 100) |
| `--hook-runs <zahl>` | Zahl der gemessenen Hook-Aufrufe je Ereignis (Standard 30, zusätzlich 3 Aufwärmläufe) |
| `--sse-runs <zahl>` | Zahl der gemessenen `sessions`-Meldungen (Standard 20) |
| `--db-runs <zahl>` | Zahl der gemessenen `dbboard`-Meldungen (Standard 10) |
| `--quick` | kleine Messung: 50 Tickets, 3 Starts, 10 Ladevorgänge, 5 Hook-Aufrufe, 5 `sessions`- und 3 `dbboard`-Meldungen – für Tests und als Rauchtest |
| `--hyperfine` | misst die Hooks mit `hyperfine`, falls es im `PATH` liegt; sonst mit einem Hinweis die eigene Messung |
| `--compare <datei>` | vergleicht das Ergebnis mit einer früheren Messung (siehe „Vergleich“) |
| `--tolerance <zahl>` | Toleranz des Vergleichs in Prozent (Standard 10) |
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
3. **Testprojekt der Hooks** (einmalig, vor den langen Messreihen, damit ein fehlender Build früh auffällt):
   Plugin-Bündel nach `<temp>/plugin` bauen, temporäres Git-Repo anlegen, mit `harness init` einrichten, am
   Server anmelden (`POST /api/projects`) und dem Bench-Board zuordnen (`PATCH /api/db/boards/<id>`). Danach prüft
   ein `harness init --json`, dass das CLI im Testprojekt genau dieses Board findet – ohne Zuordnung lädt der Hook
   keinen Board-Kontext und die Messung wäre zu günstig.
4. **Kaltstart** (`--runs` mal): Prozess starten und alle 10 ms fragen, bis `GET /api/health` und danach
   `GET /api/db/boards/<id>/view` mit `200` antworten. Beide Zeiten zählen ab dem Start des Prozesses.
5. **Warme Ladezeit**: Server einmal starten, fünf Aufwärm-Anfragen, dann `--loads` Ladevorgänge der Board-Ansicht
   nacheinander messen (jeweils samt vollständig gelesener Antwort).
6. **Speicher**: 5 s nach dem letzten Ladevorgang den Speicherbedarf (Resident Set Size) des Serverprozesses lesen.
7. **Hook-Laufzeit** (bei laufendem Server): je Ereignis 3 Aufwärmläufe und `--hook-runs` gemessene Läufe.
8. **SSE-Latenz** (bei laufendem Server): `--sse-runs` `sessions`-Meldungen und `--db-runs` `dbboard`-Meldungen.
9. Server beenden, Testprojekt und Messordner löschen.

## Hook-Laufzeit

Gemessen wird, was Claude Code wirklich startet: ein eigener Node-Prozess je Ereignis
(`packages/plugin/manifest/hooks/hooks.json` ruft `node ${CLAUDE_PLUGIN_ROOT}/lib/cli.mjs hook <ereignis>`). Die Zeit
umfasst den ganzen Aufruf samt Start von Node; die Hook-Meldung (`session_id`, `cwd`, `prompt`) geht auf die
Standardeingabe, je Lauf mit eigener Session-ID.

- Das Plugin-Bündel wird nach `<temp>/plugin` gebaut, **nie** nach `packages/plugin/dist/ai-harness` – das nutzen
  laufende Claude-Sessions. Die Anpassungen aus `custom/` bleiben dabei außen vor (`HARNESS_CUSTOM_DIR` auf einen
  leeren Ordner), damit alle Rechner dasselbe messen.
- `HARNESS_PORT` zeigt auf den laufenden Bench-Server: sonst würde `session-start` einen eigenen Server starten.
- `CLAUDE_ENV_FILE` wird entfernt, damit der Hook nicht in die Env-Datei einer laufenden Claude-Session schreibt;
  `CLAUDE_CONFIG_DIR` zeigt in den temporären Ordner. `HARNESS_HOOK_DEADLINE_MS` bleibt unverändert (3 s).
- Mit `--hook-cmd` entfällt der Bau des Bündels; das Testprojekt richtet aber weiter das CLI dieses Repos ein
  (`node packages/cli/dist/main.js init`), damit Token, Zähler und Board-Zuordnung unabhängig vom gemessenen
  Programm entstehen.
- Mit `--hyperfine` übernimmt `hyperfine` die Messreihe, falls es im `PATH` liegt (`--export-json`, Median und p95
  aus den Einzelzeiten). Alle Läufe bekommen dann dieselbe Meldung, die Session-ID wechselt also nicht. Fehlt
  `hyperfine` oder läuft es nicht durch, gilt mit einem Hinweis die eigene Messung – die Zahlen aller Plattformen
  entstehen so auf demselben Weg.

## SSE-Latenz

Gemessen wird die Zeit von einer Änderung bis zum Ereignis im Ereignisstrom `GET /api/projects/<pid>/events`
(Server-Sent Events). Der Strom wird gelesen wie in der Oberfläche: Blöcke an `\n\n` trennen, Zeilen mit `data:` als
JSON lesen, Kommentare (`: ping`) übergehen.

- **`sessions`**: ein Hook-Ereignis an `POST /api/hook` (Token des Testprojekts im Kopf `x-harness-token`) geht im
  Server sofort über den `EventHub` an alle offenen Ströme. Gemessen wird vom Absenden bis zum Eintreffen von
  `{"type":"sessions"}`; der Wert sagt also, was der Server selbst kostet (Bruchteile einer Millisekunde).
- **`dbboard`**: eine Ticketänderung (`PATCH /api/db/tickets/<id>`) findet der `BoardChangeWatcher` erst beim
  nächsten Durchlauf. **Diesen Wert bestimmt der Abfrage-Takt des Servers (2500 ms), nicht die Geschwindigkeit des
  Codes**: der Erwartungswert liegt bei der halben Taktzeit, der p95 nahe einer ganzen. Vor jeder Messung wartet das
  Skript eine zufällige Zeit (0–2500 ms), damit die Änderungen nicht immer an derselben Stelle des Takts liegen.
  Die Zahl ist deshalb streuend und taugt nur zum Vergleich der Größenordnung – nicht als feine Messgröße.

## Felder in `contract/baseline.json`

```json
{
  "schema": 1,
  "createdAt": "2026-10-27T10:00:00.000Z",
  "machine": { "os": "darwin", "release": "25.6.0", "arch": "arm64", "cpu": "Apple M…", "cpus": 10, "memGb": 32, "node": "v26.10.0" },
  "server": { "cmd": "node packages/server/dist/main.js --port {port}", "tickets": 500 },
  "hooks": { "cmd": "node <plugin>/lib/cli.mjs hook" },
  "metrics": {
    "serverColdStartMs": { "median": 0, "p95": 0, "runs": 10 },
    "boardFirstLoadMs": { "median": 0, "p95": 0, "runs": 10 },
    "boardLoadMs": { "p50": 0, "p95": 0, "mean": 0, "runs": 100 },
    "serverRssIdleMb": { "value": 0 },
    "hookSessionStartMs": { "median": 0, "p95": 0, "runs": 30 },
    "hookUserPromptMs": { "median": 0, "p95": 0, "runs": 30 },
    "sseSessionsLatencyMs": { "median": 0, "p95": 0, "runs": 20 },
    "sseDbBoardLatencyMs": { "median": 0, "p95": 0, "runs": 10 }
  }
}
```

| Feld | Bedeutung |
| --- | --- |
| `schema` | Version des Ausgabeformats |
| `createdAt` | Zeitpunkt der Messung |
| `machine` | Rechner der Messung: Betriebssystem, Ausgabe von `uname -r`, Architektur, Prozessor, Zahl der Kerne, Arbeitsspeicher in GB, Node-Version |
| `server` | gemessener Startbefehl und Zahl der Tickets im Board |
| `hooks` | gemessener Hook-Befehl; der temporäre Ordner des Plugin-Bündels wechselt je Lauf und steht deshalb als `<plugin>` darin |
| `serverColdStartMs` | Kaltstart: Millisekunden vom Start des Prozesses bis zur ersten Antwort von `/api/health` (Median, p95, Zahl der Läufe) |
| `boardFirstLoadMs` | „Board sichtbar“ auf Serverseite: Millisekunden vom Start des Prozesses bis zur ersten vollständigen Board-Ansicht (enthält den Kaltstart und das Öffnen der Datenbank) |
| `boardLoadMs` | warme Ladezeit der Board-Ansicht in Millisekunden bei laufendem Server (p50, p95, Mittelwert, Zahl der Ladevorgänge) |
| `serverRssIdleMb` | Speicherbedarf des Serverprozesses im Leerlauf in MB; `null`, wenn der Prozess nicht gefunden wurde |
| `hookSessionStartMs` | Laufzeit eines `session-start`-Hooks in Millisekunden (Median, p95, Zahl der Läufe); mit `--hyperfine` steht zusätzlich `tool: "hyperfine"` darin |
| `hookUserPromptMs` | Laufzeit eines `user-prompt`-Hooks in Millisekunden |
| `sseSessionsLatencyMs` | Latenz der `sessions`-Meldung in Millisekunden: von `POST /api/hook` bis zum Ereignis im Strom |
| `sseDbBoardLatencyMs` | Latenz der `dbboard`-Meldung in Millisekunden: von der Ticketänderung bis zum Ereignis im Strom (vom Abfrage-Takt des Servers bestimmt, siehe oben) |

Alle Werte sind auf eine Nachkommastelle gerundet. Besteht die Datei schon, ersetzt ein Lauf nur die acht Messgrößen
dieses Skripts und seine Angaben zu Rechner, Server und Hooks; fremde Einträge unter `metrics` (etwa eine spätere
Messung der Oberfläche) bleiben erhalten.

## Vergleich

`--compare <datei>` vergleicht die frische Messung mit einer früheren (`--tolerance <prozent>`, Standard 10):

- Maßgeblich ist je Messgröße `p95`, sonst `median`, sonst `value`. Größer heißt schlechter.
- Die Tabelle alt/neu/Abweichung geht auf stderr, das JSON des neuen Laufs auf stdout.
- Rückgabecode 1, sobald eine Messgröße die Toleranz überschreitet, sonst 0.
- Messgrößen, die nur in einer der beiden Messungen stehen, sind nur eine Warnung und ändern den Code nicht.
- Stammen die Messungen von verschiedenen Rechnern, steht über der Tabelle ein Hinweis: dann sagt sie wenig aus.

`--compare` und `--out` lassen sich zusammen verwenden, auch mit derselben Datei: verglichen wird gegen den Stand
vor dem Schreiben.

## Dateien

| Datei | Aufgabe |
| --- | --- |
| `bench.mjs` | Einstieg: Argumente, Ablauf, Ausgabe, Vergleich und das Zusammenführen mit einer vorhandenen Datei |
| `lib/ServerProcess.mjs` | Server starten und beenden, freier Port, auf `200` warten, einzelne Anfrage messen |
| `lib/BoardSeeder.mjs` | Board-Konfig, Projekt, Board, Tickets, Spalten und Kommentare über die HTTP-API |
| `lib/BenchProject.mjs` | Plugin-Bündel in den Temp-Ordner bauen, Testprojekt der Hooks anlegen, einrichten und zuordnen |
| `lib/HookBench.mjs` | Laufzeit eines Hook-Aufrufs (eigene Messung oder `hyperfine`) |
| `lib/SseBench.mjs` | Latenz des Ereignisstroms für `sessions` und `dbboard` |
| `lib/SseStream.mjs` | Ereignisstrom lesen und den Zeitpunkt jedes Ereignisses festhalten |
| `lib/BaselineCompare.mjs` | Vergleich zweier Messungen und die Tabelle dazu (reine Funktionen) |
| `lib/BenchApi.mjs` | Anfragen an den gemessenen Server (Pflichtkopf, Zeitlimit, Fehlertexte) |
| `lib/Stats.mjs` | Median, Perzentil, Mittelwert, Runden (reine Funktionen) |
| `lib/ProcessMemory.mjs` | Speicherbedarf eines fremden Prozesses in MB |
| `tests/*.test.mjs` | Unit-Tests der reinen Teile (`npx vitest run scripts/bench`) |

## Bekannte Grenzen

- **Nur der Server und die Hooks.** Der Speicherbedarf des Browser-Tabs (Oberfläche) wird nicht gemessen. Beim
  Vergleich mit der Tauri-Version fehlt diese Hälfte also; sie braucht eine eigene Messung.
- **Nur SQLite.** Gemessen wird gegen eine frische SQLite-Datei, nicht gegen SQL Server.
- **Der Rechner prägt die Zahlen.** Parallele Arbeit (etwa laufende Agenten) macht sie schlechter. Zahlen aus
  verschiedenen Läufen nur vergleichen, wenn `machine` übereinstimmt – am besten auf einem ruhigen Rechner messen.
- **Prozessnummer des Servers.** Der Startbefehl läuft über die Shell, deren Prozessnummer nicht zum Messen des
  Speichers taugt. Das Skript nimmt deshalb die Nummer aus `<home>/server.json` und, falls die Datei fehlt, die aus
  der Antwort von `/api/health`. Ein Server ohne beides liefert `serverRssIdleMb: null`.
- **Keine gleichzeitigen Anfragen.** Die warmen Ladevorgänge laufen nacheinander; Last durch mehrere Tabs ist nicht
  Teil dieser Messung. Auch die Hooks laufen nacheinander, nie mehrere Sessions gleichzeitig.
- **`sseDbBoardLatencyMs` misst den Takt, nicht den Code.** Der Wert folgt dem Abfrage-Takt des Servers (2500 ms)
  und streut entsprechend; nur die Größenordnung ist vergleichbar.
- **Das Testprojekt der Hooks ist leer.** Es hat einen Commit und keine Dateien; ein großes Repo mit vielen
  Worktrees kostet die Hooks mehr.
