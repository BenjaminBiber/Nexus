# Vertragstest von HTTP-API, CLI und Hooks

Die HTTP-API des Servers und das Verhalten von `harness` und seinen Hooks sind der Vertrag, den jede
Umsetzung erfüllen muss – heute der TypeScript-Server und die TypeScript-CLI, später beide in Rust.
Dieser Ordner enthält das Werkzeug dafür: Es startet einen Server mit frischem `HARNESS_HOME`, spielt
Anfragen und CLI-Aufrufe aus Szenario-Dateien ab, ersetzt die je Lauf wechselnden Werte durch Platzhalter
und vergleicht das Ergebnis mit einer Golden-Datei – oder zeichnet sie neu auf.

Alles hier ist plain Node (ESM, `.mjs`), ohne zusätzliche Abhängigkeiten und ohne eigenen Build-Schritt.
Nur der geprüfte Server und die geprüfte CLI müssen gebaut sein (`npm run build`).

Es gibt zwei Suiten: `api` (Standard) mit den HTTP-Routen und `cli` mit den Hooks und den Hilfetexten.

## Aufrufe

```sh
npm run build                      # Voraussetzung: packages/*/dist und packages/web/dist
npm run contract                   # alles vergleichen (= node contract/run.mjs)
node contract/run.mjs --update     # Golden-Dateien neu aufzeichnen
node contract/run.mjs --only basis/projects
node contract/run.mjs --coverage   # alle Routen mit abgedeckt/offen
node contract/run.mjs --suite cli             # Hooks und Hilfetexte
node contract/run.mjs --suite cli --only cli/hooks
node contract/lib/gen-help-scenarios.mjs      # Szenario der Hilfetexte neu erzeugen
npx vitest run contract            # Einheitentests (Normalisierer, Lader, CLI-Schritt, Generator)
```

| Option | Bedeutung |
| --- | --- |
| `--update` | Golden-Dateien neu schreiben; nur Dateien mit geändertem Inhalt werden angefasst. |
| `--only <teilstring>` | Nur Szenarien, deren Pfad den Teilstring enthält (z. B. `basis/health`). |
| `--suite <name>` | Ordner unter `contract/` mit den Szenarien (Standard `api`). |
| `--server-cmd "<cmd {port}>"` | Eigener Startbefehl statt des TS-Servers; `{port}` wird ersetzt. Relative Pfade gelten ab der Projektwurzel. |
| `--base <url> --home <ordner>` | Gegen einen schon laufenden Server prüfen (siehe unten). |
| `--cli-cmd "<cmd>"` | Befehl der CLI für `harnessInit` und die `cli`-Schritte (Standard `node packages/cli/dist/main.js`). Auch als JSON-Array: `'["node","/Mein Ordner/main.js"]'` – der sichere Weg für Pfade mit Leerzeichen. Relative Pfade gelten ab der Projektwurzel. |
| `--coverage` | Routen aus `api/routes.json` auflisten, Exit 0. |
| `--strict` | Zusammen mit `--coverage`: Exit 1, wenn eine Route offen ist. |
| `--verbose` | Ausgabe des gestarteten Servers mitschreiben. |

Exit-Codes: `0` alles grün, `1` Abweichung, fehlende Golden-Datei oder offene Route bei `--coverage --strict`,
`2` Fehler im Aufruf (unbekannte Option, `--base` ohne `--home`).

## Ablauf

Jedes Szenario läuft gegen einen **frischen Server mit frischem Home**. Deshalb sind Auto-Inkrement-IDs und
Ticket-IDs (`BEN-0001`) deterministisch und müssen nicht normalisiert werden. Das gilt auch für die Suite
`cli`: Die Hooks melden sich am Server des Szenarios, nicht am echten unter Port 4710.

1. Szenarien unter `contract/<suite>/**` suchen, nach `--only` und `platforms` filtern.
2. Je Szenario: temporäres `HARNESS_HOME`, temporäre `CLAUDE_CONFIG_DIR`, freier Port, Fixture-Repos anlegen.
3. Server starten (Standard `node contract/servers/ts-server.mjs --port {port}`) und auf `GET /api/health`
   warten (höchstens 15 s).
4. Fixture `boardConfig` anwenden, dann die Schritte ausführen und normalisieren.
5. Server beenden (unter Windows mit `taskkill /T /F`), Home, Repos und Claude-Konfiguration löschen.
6. Vergleichen oder (mit `--update`) aufzeichnen; am Ende eine Zusammenfassung.

## Vertragsmodus des Servers

Jede Umsetzung des Servers muss diesen Modus anbieten; nur so sind die Antworten reproduzierbar.
Beim TypeScript-Server macht das `contract/servers/ts-server.mjs`.

- Version in `GET /api/health` ist `contract`.
- Kein Takt des Dispatchers; es wird **nie** ein Worker gestartet (`POST /api/dispatch/run` bleibt wirkungslos).
- Kein Ordnerdialog (`POST /api/db/pick-folder` antwortet mit `{ "path": null }`), kein Öffnen von Dateien im
  Explorer oder in VS Code.
- Keine Meldungen über neue Fragen (Abfrage aus).
- `GET /api/claude-usage` antwortet mit `available: false`, `reason: "no-login"` und sonst `null`.
- Claude-Aufrufe starten nie ein echtes Claude, sondern liefern feste Ergebnisse: Optimierungs-Vorschläge und
  Ticket-Entwürfe `[]`, das Umschreiben eines Dokument-Bereichs `Ersatztext`.
- `CONTRACT_HEARTBEAT_MS` steuert den Herzschlag des Ereignisstroms, `CONTRACT_POLL_MS` die Abfrage-Takte
  (Board-Änderungen, Dokumente).
- Daten liegen ausschließlich unter `HARNESS_HOME`; der echte `~/.harness` und Port 4710 werden nie benutzt.

Der Server wird mit `HARNESS_HOME`, `HARNESS_PORT`, `CLAUDE_CONFIG_DIR` und den `CONTRACT_*`-Variablen in der
Umgebung gestartet und hört auf `127.0.0.1`. Er schreibt `server.json` wie der normale Start und beendet sich
bei `SIGTERM`/`SIGINT` sauber.

## Szenario-Format (`*.scenario.json`)

```json
{
  "title": "Projekte anmelden und auflisten",
  "platforms": ["darwin", "linux", "win32"],
  "fixtures": {
    "repos": [{ "name": "REPO1", "git": true, "harnessInit": false, "files": { "docs/a.md": "# A\n" } }],
    "boardConfig": "sqlite"
  },
  "steps": [
    { "name": "anmelden", "method": "POST", "path": "/api/projects", "body": { "path": "{{REPO1}}" },
      "save": { "pid": "json.id" } },
    { "name": "liste", "method": "GET", "path": "/api/projects" },
    { "name": "ohne Pflichtkopf", "method": "POST", "path": "/api/projects", "body": {},
      "headers": { "x-harness": null } }
  ]
}
```

- `title` ist Pflicht, `platforms` und `fixtures` sind freiwillig. Ohne `platforms` läuft das Szenario überall.
- Unbekannte Felder sind ein Fehler – Tippfehler fallen so beim Laden auf, nicht erst im Vergleich.
- Die Golden-Datei liegt daneben und heißt wie das Szenario: `projects.scenario.json` → `projects.golden.json`.

### Fixtures

- `repos`: temporäre Git-Repos. `name` in Großbuchstaben (`REPO1`), danach als `{{REPO1}}` verwendbar.
  `git` (Standard `true`) legt ein Repo mit erstem Commit an, `files` schreibt Dateien (Pfade mit `/`),
  `harnessInit` führt `<cli> init` im Repo aus (nötig, damit sich das Projekt anmelden lässt).
  Der Ordner heißt `<temp>/<NAME>/Mein Projekt ä` – Leerzeichen und Umlaut gehören zum Test.
- `boardConfig: "sqlite"`: vor den Schritten `PUT /api/board-config` mit
  `{ provider: "sqlite", dbFile: "<HOME>/board.db", userId: "11111111-1111-1111-1111-111111111111", userName: "Vertrag" }`.
  Dieser Aufruf wird **nicht** aufgezeichnet.
- `claudeEnvFile: true`: setzt `CLAUDE_ENV_FILE` auf `<CLAUDE>/claude-env.sh` und zeichnet den Inhalt dieser
  Datei nach jedem `cli`-Schritt als `response.envFile` auf. `session-start` schreibt dort `HARNESS_SESSION`
  und `HARNESS_TICKET` (`EnvFileWriter` in `packages/core/src/hooks/`). Ohne die Fixture gibt es kein
  `CLAUDE_ENV_FILE` und kein `envFile` in der Golden-Datei.

### Variablen

`{{NAME}}` wird in `path`, `body` und `headers` eingesetzt, auch verschachtelt. Bekannt sind:

- die Repo-Namen der Fixtures (`{{REPO1}}` → Pfad des Repos),
- `{{HOME}}` (das `HARNESS_HOME` des Laufs) und `{{PORT}}`,
- alle mit `save` gemerkten Werte.

Besteht ein Wert nur aus einem Platzhalter (`"{{PORT}}"`), behält er seinen Typ; sonst wird er in den Text
eingesetzt. Ein unbekannter Name ist ein Fehler.

`save` liest Werte aus der Antwort über Punkt-Notation, beginnend mit `json.` oder `headers.`
(`"save": { "pid": "json.id", "ort": "headers.location" }`). Listen sprechen Indizes an: `json.items.0.id`.

### Schritte

Jeder Schritt hat einen eindeutigen `name`. `kind` wählt die Art (Standard `http`), eine unbekannte Art ist
ein Fehler. `save`, `compare`, `ignore` und `record` gelten für jede Art:

| Feld | Bedeutung |
| --- | --- |
| `save` | Werte für späteren Gebrauch merken. |
| `compare` | `full` (Standard), `status` oder `shape`. |
| `ignore` | Felder, die vor dem Vergleich entfernt werden (`"json.a.b"`, relativ zur Antwort). |
| `record` | `false` führt den Schritt aus, zeichnet ihn aber nicht auf (Vorbereitung). |

Dazu die eigenen Felder der Art `http`:

| Feld | Bedeutung |
| --- | --- |
| `method` | `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`. |
| `path` | Pfad ab `/`, mit Abfrageteil erlaubt. |
| `body` | Körper als JSON; bei `GET`/`HEAD` wird er nur aufgezeichnet, nicht gesendet. |
| `headers` | Zusätzliche Köpfe. Wert `null` lässt einen Standardkopf weg (für Tests des Schutzes). |

Standardköpfe bei allem außer `GET`/`HEAD`: `x-harness: 1` und `content-type: application/json`.
Der `Host`-Kopf ist immer `127.0.0.1:<port>`, weil die Anfragen an diese Adresse gehen (`localhost` würde der
`RequestGuard` nur dann annehmen, wenn er auch so angesprochen wird). `fetch` setzt keinen `origin`; für den
Test der fremden Herkunft steht er im Szenario.

`compare`-Arten:

- `full`: Status, Inhaltstyp und Körper genau vergleichen.
- `status`: nur den Status; Inhaltstyp und Körper fallen aus der Aufzeichnung.
- `shape`: Status und Inhaltstyp genau, der Körper nur als Schlüssel und JSON-Typen (`"<string>"`, `"<number>"`,
  `"<boolean>"`, `"<null>"`). Listen werden auf ihre verschiedenen Formen zusammengefasst, damit die Länge
  nichts ausmacht; ein Text-Körper wird zu `"<string>"`.

### Schritt-Art `cli`

Ruft die CLI als eigenen Prozess auf und zeichnet Rückgabecode, Standardausgabe und Standardfehler auf
(`lib/CliStep.mjs`).

```json
{ "name": "session-start", "kind": "cli", "args": ["hook", "session-start"], "cwd": "{{REPO1}}",
  "stdin": { "session_id": "s1", "cwd": "{{REPO1}}", "source": "startup" },
  "env": { "HARNESS_TICKET": "ARC-0001", "HARNESS_HOOK_DEADLINE_MS": "60000" } }
```

| Feld | Bedeutung |
| --- | --- |
| `args` | Argumente nach dem Programm, nicht leer. `{{NAME}}` wird eingesetzt. |
| `cwd` | Arbeitsordner; ohne Angabe das `HARNESS_HOME` des Laufs. |
| `stdin` | Objekt → als JSON hinein, Text → roh (so kommen kaputtes JSON und leere Eingabe in den Vertrag). |
| `env` | Zusätzliche Umgebungsvariablen; sie gewinnen gegen die des Rahmens. |

Das Programm kommt aus `--cli-cmd` und wird in Programm und Argumente zerlegt – **ohne Shell**, damit
Leerzeichen in Pfaden sicher sind. Die Umgebung ist die geerbte, aber:

- **gesetzt**: `HARNESS_HOME` (Home des Laufs), `HARNESS_PORT` (Port des Szenario-Servers),
  `CLAUDE_CONFIG_DIR` (leerer temporärer Ordner – der echte `~/.claude` wird nie angefasst),
  `NO_COLOR=1`, `LANG=C.UTF-8`;
- **entfernt**: `HARNESS_SESSION`, `HARNESS_TICKET`, `CLAUDE_ENV_FILE` – eine geerbte Zuordnung gilt nicht
  für den Vertrag. Wer sie braucht, setzt sie im `env` des Schritts.

Der Szenario-Server läuft schon auf `HARNESS_PORT`; `session-start` erkennt ihn und startet keinen zweiten
(`probe` in `packages/cli/src/hooks/HookRunner.ts`). `HARNESS_SERVER_ENTRY` wird deshalb nicht gesetzt.

Aufgezeichnet wird `{ code, stdout | json, stderr }`, dazu `envFile` mit der Fixture `claudeEnvFile`.
Ist die Standardausgabe ein JSON-Objekt oder eine JSON-Liste, steht sie als `json` statt als `stdout` –
die Hooks antworten mit JSON, die Hilfetexte mit Text. Zusätzlich zur Normalisierung der HTTP-Antworten
werden Zeilenenden `\r\n` zu `\n` und abschließende Leerzeilen weggelassen. `request.env` steht nur dort,
wo das Szenario Variablen nennt; die des Rahmens stehen hier im README und nicht in jeder Golden-Datei.

### Schritt-Art `httpStub`: Attrappe eines fremden Dienstes

Manche Routen rufen einen Dienst außerhalb des Rechners auf – `POST /api/user-settings/test-ntfy` schickt eine
Push-Meldung an einen ntfy-Server. Diese Art startet dafür einen Mini-HTTP-Server auf einem freien Port, der
jede Anfrage mit einem festen Status beantwortet und aufzeichnet, was ankam.

| Feld | Bedeutung |
| --- | --- |
| `stub` | Name der Attrappe in Großbuchstaben (`NTFY`), Pflicht. |
| `action` | `start` (Standard) startet sie, `requests` zeichnet die empfangenen Anfragen auf. |
| `status` | Antwortstatus der Attrappe, Standard `200` (nur bei `start`). |

```json
{ "name": "Attrappe starten", "kind": "httpStub", "stub": "NTFY", "save": { "ntfyUrl": "json.url" } },
{ "name": "einrichten", "method": "PUT", "path": "/api/user-settings",
  "body": { "ntfy": true, "ntfyServer": "{{ntfyUrl}}", "ntfyTopic": "harness-vertrag" } },
{ "name": "Push-Test", "method": "POST", "path": "/api/user-settings/test-ntfy", "body": {} },
{ "name": "Anfrage an der Attrappe", "kind": "httpStub", "action": "requests", "stub": "NTFY" }
```

- `start` liefert die Adresse als `json.url`; mit `save` wird sie zur Variable und – weil sie den wechselnden
  Port enthält – in der Golden-Datei zu `<var:name>`.
- `requests` liefert je Anfrage Methode, Pfad, die Köpfe `content-type` und `authorization` und den Körper als
  `json` oder `text`. Andere Köpfe hängen am HTTP-Client und gehören nicht zum Vertrag.
- Die Attrappen leben nur in ihrem Szenario; der Runner beendet sie danach.

## Golden-Format (`*.golden.json`)

```json
[
  { "name": "anmelden",
    "request": { "method": "POST", "path": "/api/projects", "body": { "path": "<REPO1>" } },
    "response": { "status": 201, "contentType": "application/json",
                  "json": { "id": "<var:pid>", "path": "<REPO1>" } } }
]
```

Ein `cli`-Schritt sieht so aus:

```json
[
  { "name": "session-start",
    "request": { "args": ["hook", "session-start"], "cwd": "<REPO1>",
                 "env": { "HARNESS_HOOK_DEADLINE_MS": "60000" }, "stdin": { "cwd": "<REPO1>", "session_id": "s1" } },
    "response": { "code": 0, "envFile": "export HARNESS_SESSION='s1'\n",
                  "json": { "hookSpecificOutput": { "hookEventName": "SessionStart" } }, "stderr": "" } }
]
```

- Eine Liste in der Reihenfolge der aufgezeichneten Schritte, je Schritt `name`, `request` und `response`.
- `request.headers` steht nur dort, wo das Szenario Köpfe nennt; `null` zeigt einen bewusst fehlenden Kopf.
- `contentType` ohne Parameter (`; charset=…` entfällt), `null` wenn die Antwort keinen hat.
- Der Körper steht als `json`, sonst als `text` (Zeilenenden auf `\n`); ein leerer Körper ist `"text": ""`.
- Objekt-Schlüssel sind sortiert (stabile Diffs), Listen behalten ihre Reihenfolge.
  2 Leerzeichen Einrückung, `\n` am Ende der Datei.

Abweichungen melden Szenario, Schritt und einen zeilenweisen Diff (`-` Golden-Datei, `+` aufgezeichnet).
Fehlt die Golden-Datei, scheitert der Lauf mit dem Hinweis auf `--update`.

## Normalisierung

Alle Textwerte – auch Objekt-Schlüssel – gehen durch diese Regeln, in dieser Reihenfolge:

1. **Gespeicherte Werte** aus `save` (nur Texte ab 6 Zeichen) → `<var:name>`.
2. **Pfade**: Repo-Ordner → `<REPO1>` …, `HARNESS_HOME` → `<HOME>`, `CLAUDE_CONFIG_DIR` → `<CLAUDE>`,
   der Ordner der Fixture-Repos → `<REPOS>`, der Temp-Ordner → `<TMP>`. Jeder Pfad in drei Schreibweisen
   (nativ, mit `/`, JSON-escaped mit `\\`), der längste Pfad zuerst; unter macOS zählt auch die aufgelöste
   Form (`/private/var/…` ↔ `/var/…`). Der Rest des Pfades hinter dem Platzhalter bekommt Schrägstriche,
   damit die Golden-Datei unter Windows dieselbe ist.
3. **Port** des Servers: `127.0.0.1:<port>` und `localhost:<port>` → `…:<PORT>`.
4. **Zeit**: ISO-Zeitstempel → `<TIME>`, ebenso Datum mit Uhrzeit in deutscher Schreibweise
   (`27.10.2026, 18:05`) – so gibt die CLI Kommentare und Verlauf aus. Ein Datum `YYYY-MM-DD` nur dann →
   `<DATE>`, wenn es das heutige ist.
5. **Datum relativ zu heute**: ein Datum bis zu einem Jahr vor heute → `<DATE-55>` (die Zahl ist der Abstand in
   Tagen). Das trifft Grenzen, die der Server aus „heute“ rechnet – etwa der Anfang des Standard-Zeitraums der
   Dispatch-Statistik (acht Wochen). Feste Daten eines Szenarios liegen deshalb weiter zurück (`2020-03-02`);
   sonst hinge die Golden-Datei am Tag der Aufzeichnung.
6. **UUIDs** → `<UUID>`, außer `11111111-1111-1111-1111-111111111111` (der Vertrags-Benutzer).
7. **Git-Commits** in Feldern `sha`, `commit`, `head` (40 bzw. 7–12 Hex-Zeichen) → `<SHA>`.
8. **Zahlenfelder** mit Namen auf `Ms` oder `At` sowie `durationMs`, `pid` und `port` → `0`. Das trifft auch
   feste Werte wie den Standard-Port 1433 der Board-Konfiguration; dafür bleibt die Datei über Läufe gleich.

Jede Regel ist eine eigene Methode in `lib/Normalizer.mjs` und einzeln getestet
(`tests/Normalizer.test.mjs`).

## Routenliste und Abdeckung

`api/routes.json` enthält alle Routen des Servers (Methode, Muster, Quelldatei) und wird aus
`packages/server/src/routes/*.ts` erzeugt:

```sh
node contract/lib/extract-routes.mjs
```

Der Rückfall der Oberfläche hat kein Muster im Quellcode (`app.setNotFoundHandler`); er steht als `GET /*`
in der Liste und deckt alles ab, was `StaticRoutes.ts` neben `/` beantwortet: Seiten der Oberfläche, Dateien
unter `assets/` und unbekannte Adressen.

`--coverage` liest die Golden-Dateien (es läuft kein Server), ordnet jeden aufgezeichneten Schritt über
Methode und Pfad einem Muster zu (`:param` → ein Pfadabschnitt, `*` → Rest des Pfades, Abfrageteil
abgeschnitten) und gibt je Quelldatei eine Tabelle aus: `[x]` abgedeckt, `[ ]` offen. Ein Muster mit `*`
zählt erst, wenn keine genauere Route passt. Schritte ohne passende Route stehen am Ende mit `?` – dann ist
`routes.json` veraltet. Mit `--strict` endet der Lauf mit Exit 1, solange eine Route offen ist (gedacht für
den Abschluss von Phase 0).

`--coverage` gilt nur für die Suite `api`; die Suite `cli` kennt keine Routenliste.

## Suite `cli`: Hooks und Hilfetexte

```sh
node contract/run.mjs --suite cli --only cli/hooks
node contract/run.mjs --suite cli --only cli/help
```

### Hooks (`cli/hooks/`)

Je Hook-Ereignis ein Szenario: Fixture-Repo mit `harness init`, SQLite-Board, Board dem Repo zugeordnet,
ein Ticket in Arbeit mit einem ungelesenen Kommentar. Das Szenario ruft den Hook auf und liest danach
`GET /api/projects/<pid>/sessions` – so steht sowohl die Ausgabe an Claude Code als auch die Wirkung auf
den Server im Vertrag.

| Szenario | Was es festhält |
| --- | --- |
| `session-start` | Leitfaden und Kommentar als `additionalContext`, `HARNESS_SESSION` in der Umgebungsdatei, Session am Server. Beim zweiten Aufruf fehlt der Kommentar: er gilt als gelesen. |
| `user-prompt` | Kommentar genau einmal, Session-Name aus dem Prompt (`SE-001: …`). |
| `notification`, `stop`, `subagent-start`, `subagent-stop`, `session-end` | still (kein `additionalContext`), aber gemeldet: Zustand, Subagenten und Ende der Session. |
| `stille-eingaben` | kaputtes JSON, leere Eingabe, `[]`, `null`, fehlende und nicht-textuelle Sitzungskennung, unbekanntes Ereignis – alle Exit 0, keine Ausgabe, keine Session. |
| `ohne-board` | Ohne `board.yaml` gibt es keinen Kontext, die Session wird trotzdem gemeldet. Im Repo ohne `harness init` passiert nichts. |
| `zuordnung` | Ohne `HARNESS_TICKET` erst „keinem Ticket zugeordnet“, dann über die verknüpfte Session; ein unbekanntes `HARNESS_TICKET` fällt darauf zurück. |
| `worktree` | Hook aus `<REPO1>-worktrees/ARC-0001`: das Projekt bleibt das Hauptrepo (gemeinsames Git-Verzeichnis), das Ticket kommt aus dem Ordner. |

`HARNESS_HOOK_DEADLINE_MS` steht in jedem Szenario großzügig (60 s). Die Frist selbst ist **kein**
Vertragsbestandteil – sie ist nur hoch, damit ein langsamer CI-Rechner den Hook nicht abbricht.

### Hilfetexte (`cli/help/`)

`cli/help/help.scenario.json` ist **erzeugt**, nicht von Hand geschrieben:

```sh
node contract/lib/gen-help-scenarios.mjs
node contract/run.mjs --suite cli --only cli/help --update
```

Der Generator ruft `<cli> --help` auf, liest aus dem Abschnitt „Befehle“ rekursiv alle Unterbefehle
(Argumente und Aliasse werden abgeschnitten) und schreibt je Befehl einen Schritt `{"kind":"cli","args":[…,"--help"]}`.
Dazu kommen die Fehlermeldungen von commander (`GermanErrors`): unbekannter Befehl, mit Vorschlag
„Meinten Sie …?“, unbekannter Unterbefehl, fehlendes Pflichtargument, unbekannte Option am Programm und an
einem Befehl, Option ohne Wert. Der eingebaute Befehl `help` bekommt keinen eigenen Schritt – er zeigt die
Hilfe des übergeordneten Befehls; stattdessen prüft ein Schritt `harness help board`.

Zwei Dinge gehören zum Vertrag und sind leicht zu übersehen:

- **Zeilenumbruch bei 80 Zeichen.** Die Ausgabe geht in eine Pipe, nicht auf ein Terminal; commander bricht
  dann bei 80 Zeichen um. Der Port muss genauso umbrechen.
- **Der Programmname `harness`** steht in jedem Hilfetext (`Aufruf: harness board …`) und wird **nicht**
  normalisiert: Der Alias `harness` bleibt Vertrag. Wird der Port als `nexus` aufgerufen, darf er den Namen
  tauschen; dann kommt hier eine Normalisierungsregel dazu – vorher nicht.

## Gegen einen fremden Server prüfen

```sh
H=$(mktemp -d)
HARNESS_HOME=$H node contract/servers/ts-server.mjs --port 47999 &
node contract/run.mjs --base http://127.0.0.1:47999 --home $H --only basis/health
kill %1
```

Der fremde Server muss mit genau diesem leeren Home gestartet sein. Weil sein Zustand nicht zurückgesetzt
werden kann, ist **genau ein** Szenario erlaubt: `--only` muss eindeutig passen, sonst bricht der Lauf ab.
Dieselbe Form nimmt später der Server in Rust: entweder `--base` auf einen von Hand gestarteten Server oder
`--server-cmd "target/debug/harness-server --port {port}"`, damit `run.mjs` ihn selbst startet.

## Dateien

```
contract/
├─ README.md
├─ run.mjs                    # Einstieg: Argumente, Ablauf, Exit-Code
├─ lib/
│  ├─ ScenarioLoader.mjs      # liest *.scenario.json, prüft die Felder
│  ├─ ScenarioRunner.mjs      # Home, Fixtures, Variablen, Schritte, Normalisierung
│  ├─ StepKinds.mjs           # Registry der Schritt-Arten
│  ├─ HttpStep.mjs            # Schritt-Art „http“
│  ├─ CliStep.mjs             # Schritt-Art „cli“ (Programm, Umgebung, stdin)
│  ├─ HttpStubStep.mjs        # Schritt-Art „httpStub“
│  ├─ HttpStub.mjs            # Mini-HTTP-Server der Attrappe
│  ├─ ServerHandle.mjs        # Server starten/beenden oder --base nutzen
│  ├─ Normalizer.mjs          # ersetzt die wechselnden Werte
│  ├─ GoldenStore.mjs         # lesen, schreiben, vergleichen
│  ├─ JsonText.mjs            # kanonisches JSON und zeilenweiser Diff
│  ├─ Variables.mjs           # {{NAME}} und save
│  ├─ Coverage.mjs            # routes.json gegen die aufgezeichneten Schritte
│  ├─ RepoFixture.mjs         # temporäre Git-Repos
│  ├─ extract-routes.mjs      # erzeugt api/routes.json
│  └─ gen-help-scenarios.mjs  # erzeugt cli/help/help.scenario.json
├─ servers/
│  └─ ts-server.mjs           # TS-Server im Vertragsmodus
├─ api/
│  ├─ routes.json             # alle Routen des Servers
│  ├─ basis/                  # Beispielszenarien
│  ├─ board/                  # Board-Kern: Projekte, Boards, Tickets, Schritte (/api/db)
│  ├─ board-views/            # Notizen, Suche, Statistik, Graph, Inbox
│  ├─ questions/              # Fragen-Routen
│  └─ misc/                   # Sitzungen, Favoriten, Hook, Einstellungen, Usage, Workflow, Oberfläche
├─ cli/
│  ├─ hooks/                  # je Hook-Ereignis ein Szenario
│  └─ help/                   # Hilfetexte aller Befehle (erzeugt)
└─ tests/                     # Einheitentests (vitest, siehe vitest.config.ts)
```

## Neue Szenarien und neue Schritt-Arten

Ein neues Szenario ist eine Datei unter `contract/<suite>/<gruppe>/`; die Golden-Datei entsteht mit
`node contract/run.mjs --suite <suite> --only <gruppe>/<name> --update`. Danach einmal ohne `--update`
laufen lassen – `git status` darf nichts mehr zeigen.

Eine neue Schritt-Art (geplant: Ereignisstrom) ist ein eigenes Modul mit einer Klasse, einer
Methode `run(step, context)` und einem Getter `keys` mit ihren eigenen Feldern. `context` hält `base`
(Adresse des Servers), `variables` und `stubs` (die laufenden Attrappen), für CLI-Aufrufe zusätzlich `cli`,
`home`, `claudeConfig`, `port` und `envFile`. Der Rückgabewert ist `{ request, response }`;
`response` darf alles enthalten, was die Art aufzeichnet, und `save` sowie `ignore`
greifen darauf zu (`json.…`, `headers.…`). Die Art meldet sich in der Registry `stepKinds` in
`lib/StepKinds.mjs` an; der Lader prüft dann die gemeinsamen Felder und die aus `keys`. Außer `http` und
`cli` bekommt eine Art den Schritt unverändert als `step.raw` – sie prüft ihre eigenen Felder selbst.
